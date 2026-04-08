use libadwaita::prelude::*;
use libadwaita::{ApplicationWindow, HeaderBar, Toast, ToastOverlay, ExpanderRow, ActionRow, MessageDialog, ResponseAppearance, PreferencesGroup, StatusPage};
use gtk4::{Box, ListBox, Orientation, ScrolledWindow, Label, Button, Align, SelectionMode, MenuButton, gio, Spinner, ProgressBar, Image, gdk, LinkButton, SearchEntry};
use gtk4::pango::EllipsizeMode;
use gtk4::glib;
use std::sync::{Arc, Mutex};
use std::process::Command;
use std::fs;
use crate::steam_scanner::{SteamGame, GameType, AppCategory, discover_steam_libraries, scan_games};
use crate::injector::Injector;

#[derive(Clone, Copy, PartialEq)]
enum Language { EN, RU }

struct Translations {
    title: &'static str, scanning: &'static str, downloading_core: &'static str,
    games_not_found: &'static str, apply_patch: &'static str, restore: &'static str,
    status_patched: &'static str, status_safe: &'static str, status_warn: &'static str,
    status_forbidden: &'static str, open_folder: &'static str, copy_params: &'static str,
    dlc_list_title: &'static str, view_store: &'static str, view_protondb: &'static str,
    group_games: &'static str, group_system: &'static str, confirm_title: &'static str,
    confirm_body: &'static str, confirm_yes: &'static str, confirm_no: &'static str,
    search_placeholder: &'static str,
}

const EN_TRANS: Translations = Translations {
    title: "VaporDose", scanning: "Scanning Games...", downloading_core: "Updating SmokeAPI...",
    games_not_found: "No games found.", apply_patch: "Patch DLC", restore: "Restore", status_patched: "Patched",
    status_safe: "Safe", status_warn: "Risk", status_forbidden: "Forbidden", open_folder: "Folder",
    copy_params: "Copy", dlc_list_title: "DLC IDs", view_store: "Store", view_protondb: "ProtonDB",
    group_games: "Installed Games", group_system: "System Tools", confirm_title: "Warning",
    confirm_body: "Online features detected. Proceed?", confirm_yes: "Proceed",
    confirm_no: "Cancel", search_placeholder: "Filter games...",
};

const RU_TRANS: Translations = Translations {
    title: "VaporDose", scanning: "Загрузка игр...", downloading_core: "Обновление ядра...",
    games_not_found: "Не найдено.", apply_patch: "Патчить", restore: "Вернуть", status_patched: "Готово",
    status_safe: "Безопасно", status_warn: "Риск", status_forbidden: "Запрещено", open_folder: "Папка",
    copy_params: "Копировать", dlc_list_title: "ID DLC", view_store: "Магазин", view_protondb: "ProtonDB",
    group_games: "Установленные игры", group_system: "Системные компоненты", confirm_title: "Внимание",
    confirm_body: "В игре есть онлайн-функции. Продолжить?", confirm_yes: "Да",
    confirm_no: "Отмена", search_placeholder: "Поиск игр...",
};

pub fn build_ui(app: &libadwaita::Application) {
    let current_lang = Arc::new(Mutex::new(Language::RU));
    let injector = Arc::new(Injector::new());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("VaporDose")
        .default_width(950)
        .default_height(850)
        .build();

    let toast_overlay = ToastOverlay::new();
    let main_box = Box::new(Orientation::Vertical, 0);
    let header_bar = HeaderBar::new();
    main_box.append(&header_bar);

    let search_box = Box::builder().margin_top(12).margin_start(24).margin_end(24).build();
    let search_entry = SearchEntry::builder().hexpand(true).build();
    search_box.append(&search_entry);
    main_box.append(&search_box);

    let scrolled_window = ScrolledWindow::builder().vexpand(true).build();
    let content_box = Box::builder().orientation(Orientation::Vertical).margin_top(12).margin_bottom(24).margin_start(24).margin_end(24).spacing(24).build();
    let games_group = PreferencesGroup::builder().build();
    let games_list = ListBox::builder().selection_mode(SelectionMode::None).css_classes(vec!["boxed-list".to_string()]).build();
    games_group.add(&games_list);
    let system_group = PreferencesGroup::builder().build();
    let system_list = ListBox::builder().selection_mode(SelectionMode::None).css_classes(vec!["boxed-list".to_string()]).build();
    system_group.add(&system_list);
    content_box.append(&games_group); content_box.append(&system_group);

    let loading_box = Box::builder().orientation(Orientation::Vertical).spacing(20).valign(Align::Center).halign(Align::Center).build();
    let spinner = Spinner::builder().spinning(true).width_request(64).height_request(64).build();
    let loading_label = Label::builder().css_classes(vec!["title-1".to_string()]).build();
    let progress_bar = ProgressBar::builder().width_request(400).build();
    loading_box.append(&spinner); loading_box.append(&loading_label); loading_box.append(&progress_bar);

    scrolled_window.set_child(Some(&loading_box)); main_box.append(&scrolled_window);
    toast_overlay.set_child(Some(&main_box)); window.set_content(Some(&toast_overlay));

    let run_scan = glib::clone!(@weak games_group, @weak system_group, @weak games_list, @weak system_list, @weak scrolled_window, @weak content_box, @weak loading_box, @weak loading_label, @weak progress_bar, @weak window, @weak search_entry, @weak toast_overlay, @strong injector, @strong current_lang => move || {
        let lang = *current_lang.lock().unwrap();
        let trans = if lang == Language::RU { &RU_TRANS } else { &EN_TRANS };
        window.set_title(Some(trans.title));
        loading_label.set_label(trans.scanning);
        search_entry.set_placeholder_text(Some(trans.search_placeholder));
        games_group.set_title(trans.group_games);
        system_group.set_title(trans.group_system);
        progress_bar.set_fraction(0.1);
        scrolled_window.set_child(Some(&loading_box));

        let current_lang_inner = current_lang.clone();
        let toast_overlay_inner = toast_overlay.clone();
        let injector_inner = injector.clone();

        glib::spawn_future_local(async move {
            let libraries = tokio::task::spawn_blocking(discover_steam_libraries).await.unwrap_or_default();
            let games = scan_games(&libraries).await;
            progress_bar.set_fraction(0.5);
            loading_label.set_label(if *current_lang_inner.lock().unwrap() == Language::RU { RU_TRANS.downloading_core } else { EN_TRANS.downloading_core });
            glib::spawn_future_local(async move { let _ = crate::updater::check_and_download_core().await; });

            while let Some(child) = games_list.first_child() { games_list.remove(&child); }
            while let Some(child) = system_list.first_child() { system_list.remove(&child); }

            if games.is_empty() {
                let status_page = StatusPage::builder().icon_name("system-search-symbolic").build();
                scrolled_window.set_child(Some(&status_page));
            } else {
                scrolled_window.set_child(Some(&content_box));
                for game in games {
                    let row = create_game_row(game.clone(), injector_inner.clone(), toast_overlay_inner.clone(), current_lang_inner.clone(), window.clone());
                    if game.category == AppCategory::SystemTool { system_list.append(&row); }
                    else { games_list.append(&row); }
                }
            }
            progress_bar.set_fraction(1.0);
        });
    });

    let run_scan_arc = Arc::new(run_scan);
    search_entry.connect_search_changed(glib::clone!(@weak games_list, @weak system_list, @weak games_group, @weak system_group => move |entry| {
        let text = entry.text().to_lowercase();
        let filter = |list: &ListBox, group: &PreferencesGroup| {
            let mut any_visible = false;
            let mut child = list.first_child();
            while let Some(widget) = child {
                if let Some(row) = widget.downcast_ref::<ExpanderRow>() {
                    let matches = text.is_empty() || row.title().to_lowercase().contains(&text);
                    row.set_visible(matches); if matches { any_visible = true; }
                }
                child = widget.next_sibling();
            }
            group.set_visible(any_visible || text.is_empty());
        };
        filter(&games_list, &games_group); filter(&system_list, &system_group);
    }));

    let refresh_run = run_scan_arc.clone();
    let setup_lang = glib::clone!(@weak app, @strong current_lang => move |lang: Language| {
        *current_lang.lock().unwrap() = lang;
        refresh_run();
    });

    let s_ru = setup_lang.clone(); app.add_action(&{let a = gio::SimpleAction::new("set_lang_ru", None); a.connect_activate(move |_, _| s_ru(Language::RU)); a});
    let s_en = setup_lang.clone(); app.add_action(&{let a = gio::SimpleAction::new("set_lang_en", None); a.connect_activate(move |_, _| s_en(Language::EN)); a});

    let lang_menu_btn = MenuButton::builder().icon_name("view-more-symbolic").menu_model(&{
        let m = gio::Menu::new();
        m.append(Some("Русский"), Some("app.set_lang_ru"));
        m.append(Some("English"), Some("app.set_lang_en"));
        m
    }).build();
    header_bar.pack_start(&lang_menu_btn);

    let refresh_button = Button::builder().icon_name("view-refresh-symbolic").build();
    let scan_trigger = run_scan_arc.clone();
    refresh_button.connect_clicked(move |_| scan_trigger());
    header_bar.pack_end(&refresh_button);

    run_scan_arc();
    window.present();
}

fn create_game_row(game: SteamGame, injector: Arc<Injector>, toast_overlay: ToastOverlay, current_lang: Arc<Mutex<Language>>, window: ApplicationWindow) -> ExpanderRow {
    let lang_val = *current_lang.lock().unwrap();
    let trans = if lang_val == Language::RU { &RU_TRANS } else { &EN_TRANS };
    let row = ExpanderRow::builder().title(game.name.clone()).subtitle(game.install_dir.to_string_lossy()).build();

    if let Some(child) = row.first_child() {
        if let Some(box_widget) = child.downcast_ref::<Box>() {
            let mut next = box_widget.first_child();
            while let Some(w) = next {
                if let Some(lbl) = w.downcast_ref::<Label>() { lbl.set_ellipsize(EllipsizeMode::End); lbl.set_max_width_chars(45); }
                next = w.next_sibling();
            }
        }
    }

    let prefix_box = Box::new(Orientation::Horizontal, 12);
    let icon_img = Image::builder().pixel_size(48).build();
    if let Some(path) = &game.icon_path { icon_img.set_from_file(Some(path)); }
    else { icon_img.set_icon_name(Some("input-gaming-symbolic")); }
    prefix_box.append(&icon_img);

    let folder_btn = Button::builder().icon_name("folder-open-symbolic").css_classes(vec!["flat".to_string()]).tooltip_text(trans.open_folder).build();
    let dir_clone = game.install_dir.clone();
    folder_btn.connect_clicked(move |_| { Command::new("xdg-open").arg(&dir_clone).spawn().ok(); });
    prefix_box.append(&folder_btn);
    row.add_prefix(&prefix_box);

    let suffix_box = Box::new(Orientation::Horizontal, 8);
    suffix_box.set_valign(Align::Center);
    let (status_color, status_text, can_patch) = match game.category {
        AppCategory::SystemTool => ("error", trans.group_system, false),
        _ => ("success", trans.status_safe, true),
    };
    let status_dot = Label::builder().label("●").css_classes(vec![status_color.to_string(), "title-1".to_string()]).tooltip_text(status_text).build();
    suffix_box.append(&status_dot);
    let apply_btn = Button::builder().css_classes(vec!["suggested-action".to_string()]).build();
    let restore_btn = Button::builder().label(trans.restore).css_classes(vec!["destructive-action".to_string()]).build();
    let copy_btn = Button::builder().icon_name("edit-copy-symbolic").tooltip_text(trans.copy_params).build();
    suffix_box.append(&apply_btn); suffix_box.append(&restore_btn); suffix_box.append(&copy_btn);
    row.add_suffix(&suffix_box);

    let details_group = PreferencesGroup::builder().margin_top(12).margin_bottom(12).build();
    let dlc_text = if game.dlc_list.is_empty() { "None".to_string() } else { game.dlc_list.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", ") };
    let dlc_row = ActionRow::builder().title(trans.dlc_list_title).subtitle(dlc_text).build();
    details_group.add(&dlc_row);
    let links_box = Box::new(Orientation::Horizontal, 12);
    links_box.set_margin_start(12); links_box.set_margin_bottom(12);
    let store_btn = LinkButton::with_label(&format!("https://store.steampowered.com/app/{}", game.appid), trans.view_store);
    let proton_btn = LinkButton::with_label(&format!("https://www.protondb.com/app/{}", game.appid), trans.view_protondb);
    links_box.append(&store_btn); links_box.append(&proton_btn);
    let expanded_box = Box::new(Orientation::Vertical, 0);
    expanded_box.append(&details_group); expanded_box.append(&links_box);
    row.add_row(&expanded_box);

    let game_type_val = game.game_type.clone();
    let update_row_state = glib::clone!(@weak apply_btn, @weak restore_btn, @weak copy_btn, @strong current_lang => move |is_patched: bool| {
        let lang = *current_lang.lock().unwrap();
        let trans = if lang == Language::RU { &RU_TRANS } else { &EN_TRANS };
        if is_patched {
            apply_btn.set_label(trans.status_patched); apply_btn.set_sensitive(false);
            restore_btn.set_sensitive(true); copy_btn.set_visible(game_type_val == GameType::Proton);
        } else {
            apply_btn.set_label(trans.apply_patch); apply_btn.set_sensitive(can_patch);
            restore_btn.set_sensitive(false); copy_btn.set_visible(false);
        }
    });
    update_row_state(game.is_patched);

    let injector_c = injector.clone(); let game_c = game.clone(); let overlay_c = toast_overlay.clone(); let lang_c = current_lang.clone();
    copy_btn.connect_clicked(move |_| {
        if let Some(instr) = injector_c.get_proton_instructions(&game_c) {
            if let Some(display) = gdk::Display::default() { display.clipboard().set_text(&instr); overlay_c.add_toast(Toast::new(if *lang_c.lock().unwrap() == Language::RU { "Скопировано!" } else { "Copied!" })); }
        }
    });

    let injector_p = injector.clone(); let game_p = game.clone(); let overlay_p = toast_overlay.clone(); 
    let apply_btn_p = apply_btn.clone(); let restore_btn_p = restore_btn.clone(); let copy_btn_p = copy_btn.clone();
    let window_p = window.clone(); let lang_p = current_lang.clone();
    apply_btn.connect_clicked(move |_| {
        let injector = injector_p.clone(); let game = game_p.clone(); let overlay = overlay_p.clone();
        let apply_btn = apply_btn_p.clone(); let restore_btn = restore_btn_p.clone(); let copy_btn = copy_btn_p.clone();
        let lang_f = lang_p.clone();
        let proceed = move || {
            let injector_i = injector.clone(); let game_i = game.clone(); let overlay_i = overlay.clone();
            let apply_btn_i = apply_btn.clone(); let restore_btn_i = restore_btn.clone(); let copy_btn_i = copy_btn.clone();
            let lang_i = lang_f.clone();
            glib::spawn_future_local(async move {
                let game_t = game_i.clone();
                let res = tokio::task::spawn_blocking(move || injector_i.backup_and_deploy(&game_t)).await.unwrap();
                if res.is_ok() { 
                    let t = if *lang_i.lock().unwrap() == Language::RU { &RU_TRANS } else { &EN_TRANS };
                    apply_btn_i.set_label(t.status_patched); apply_btn_i.set_sensitive(false);
                    restore_btn_i.set_sensitive(true); copy_btn_i.set_visible(game_i.game_type == GameType::Proton);
                    overlay_i.add_toast(Toast::new(if *lang_i.lock().unwrap() == Language::RU { "Успешно!" } else { "Success!" })); 
                }
            });
        };
        if game_p.is_online_multiplayer {
            let t = if *lang_p.lock().unwrap() == Language::RU { &RU_TRANS } else { &EN_TRANS };
            let dialog = MessageDialog::builder().transient_for(&window_p).heading(t.confirm_title).body(t.confirm_body).build();
            dialog.add_response("cancel", t.confirm_no); dialog.add_response("proceed", t.confirm_yes);
            dialog.set_response_appearance("proceed", ResponseAppearance::Destructive);
            let proceed_t = proceed;
            dialog.connect_response(None, move |d, res| { if res == "proceed" { proceed_t(); } d.close(); });
            dialog.present();
        } else { proceed(); }
    });

    let injector_r = injector.clone(); let game_r = game.clone(); let overlay_r = toast_overlay.clone();
    let apply_btn_r = apply_btn.clone(); let restore_btn_r = restore_btn.clone(); let copy_btn_r = copy_btn.clone();
    let lang_r = current_lang.clone();
    restore_btn.connect_clicked(move |_| {
        let injector_i = injector_r.clone(); let game_i = game_r.clone(); let overlay_i = overlay_r.clone();
        let apply_btn_i = apply_btn_r.clone(); let restore_btn_i = restore_btn_r.clone(); let copy_btn_i = copy_btn_r.clone();
        let lang_i = lang_r.clone();
        glib::spawn_future_local(async move {
            let res = tokio::task::spawn_blocking(move || injector_i.restore_original(&game_i)).await.unwrap();
            if res.is_ok() { 
                let t = if *lang_i.lock().unwrap() == Language::RU { &RU_TRANS } else { &EN_TRANS };
                apply_btn_i.set_label(t.apply_patch); apply_btn_i.set_sensitive(true);
                restore_btn_i.set_sensitive(false); copy_btn_i.set_visible(false);
                overlay_i.add_toast(Toast::new(if *lang_i.lock().unwrap() == Language::RU { "Восстановлено!" } else { "Restored!" })); 
            }
        });
    });
    row
}
