use libadwaita::prelude::*;
use libadwaita::{ApplicationWindow, HeaderBar, StatusPage, Toast, ToastOverlay, ExpanderRow, ActionRow};
use gtk4::{Box, ListBox, Orientation, ScrolledWindow, Label, Button, Align, SelectionMode, PolicyType};
use gtk4::glib;
use std::sync::Arc;
use crate::steam_scanner::{SteamGame, GameType, AppCategory, discover_steam_libraries, scan_games};
use crate::injector::Injector;

pub fn build_ui(app: &libadwaita::Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Steam Compatibility Manager")
        .default_width(700)
        .default_height(600)
        .build();

    let toast_overlay = ToastOverlay::new();
    let main_box = Box::new(Orientation::Vertical, 0);
    
    let header_bar = HeaderBar::new();
    main_box.append(&header_bar);

    let update_button = Button::builder()
        .icon_name("system-software-update-symbolic")
        .label("Обновить SmokeAPI")
        .tooltip_text("Проверить и скачать обновления ядра SmokeAPI")
        .build();
    header_bar.pack_end(&update_button);

    let overlay_clone = toast_overlay.clone();
    let update_btn_clone = update_button.clone();
    update_button.connect_clicked(move |_| {
        let overlay_task = overlay_clone.clone();
        let btn_task = update_btn_clone.clone();
        
        btn_task.set_sensitive(false);
        overlay_task.add_toast(Toast::new("Проверка обновлений с GitHub..."));

        glib::spawn_future_local(async move {
            match crate::updater::check_and_download_core().await {
                Ok(msg) => {
                    overlay_task.add_toast(Toast::new(&msg));
                }
                Err(e) => {
                    overlay_task.add_toast(Toast::new(&format!("Ошибка обновления: {}", e)));
                }
            }
            btn_task.set_sensitive(true);
        });
    });

    let overlay_auto = toast_overlay.clone();
    let btn_auto = update_button.clone();
    glib::spawn_future_local(async move {
        btn_auto.set_sensitive(false);
        overlay_auto.add_toast(Toast::new("Проверка обновлений SmokeAPI..."));
        match crate::updater::check_and_download_core().await {
            Ok(msg) => {
                if !msg.contains("не требуется") {
                    overlay_auto.add_toast(Toast::new(&msg));
                }
            }
            Err(e) => {
                overlay_auto.add_toast(Toast::new(&format!("Ошибка авто-обновления: {}", e)));
            }
        }
        btn_auto.set_sensitive(true);
    });

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build();

    let status_page = StatusPage::builder()
        .title("Scanning Games...")
        .description("Идёт поиск игр и классификация (Steam Web API)...")
        .icon_name("system-search-symbolic")
        .build();

    let list_box = ListBox::builder()
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();

    scrolled_window.set_child(Some(&status_page));
    main_box.append(&scrolled_window);
    toast_overlay.set_child(Some(&main_box));
    window.set_content(Some(&toast_overlay));

    let injector = Arc::new(Injector::new());
    
    glib::spawn_future_local(async move {
        let libraries = tokio::task::spawn_blocking(discover_steam_libraries).await.unwrap_or_default();
        let games = scan_games(&libraries).await; // Now this is async

        if games.is_empty() {
            status_page.set_title("Игры не найдены");
            status_page.set_description(Some("Убедитесь, что Steam установлен и есть скачанные игры."));
            status_page.set_icon_name(Some("view-filter-symbolic"));
        } else {
            scrolled_window.set_child(Some(&list_box));
            for game in games {
                let row = create_game_row(game, injector.clone(), toast_overlay.clone());
                list_box.append(&row);
            }
        }
    });

    window.present();
}

fn create_game_row(game: SteamGame, injector: Arc<Injector>, toast_overlay: ToastOverlay) -> ExpanderRow {
    let row = ExpanderRow::builder()
        .title(game.name.clone())
        .subtitle(game.install_dir.to_string_lossy())
        .build();

    let icon_text = match game.game_type {
        GameType::Native => "🐧",
        GameType::Proton => "⚛️",
        GameType::Mixed => "🐧⚛️",
        GameType::Unknown => "❓",
    };
    
    let icon_label = Label::builder()
        .label(icon_text)
        .css_classes(vec!["title-2".to_string()])
        .build();
    row.add_prefix(&icon_label);

    let (cat_text, can_patch) = match game.category {
        AppCategory::SystemTool => ("⚙️ Системная утилита", false),
        AppCategory::FreeToPlay => ("🆓 Бесплатная игра (Осторожно)", true),
        AppCategory::NoDLC => ("🚫 Нет DLC", false),
        AppCategory::DrmFree => ("🟢 DRM-Free", false),
        AppCategory::PaidWithDLC => ("🎮 Доступен патч", true),
        AppCategory::Unknown => ("❓ Неизвестно", true),
    };

    let cat_label = Label::builder()
        .label(cat_text)
        .margin_end(12)
        .build();
    row.add_prefix(&cat_label);

    let buttons_box = Box::new(Orientation::Horizontal, 8);
    buttons_box.set_valign(Align::Center);

    let apply_button = Button::builder()
        .label("Apply Patch")
        .sensitive(can_patch)
        .css_classes(vec!["suggested-action".to_string()])
        .build();

    let restore_button = Button::builder()
        .label("Restore")
        .css_classes(vec!["destructive-action".to_string()])
        .build();

    buttons_box.append(&apply_button);
    buttons_box.append(&restore_button);

    let game_clone = game.clone();
    let overlay_clone = toast_overlay.clone();
    let injector_clone = injector.clone();
    
    apply_button.connect_clicked(move |_| {
        let game_task = game_clone.clone();
        let injector_task = injector_clone.clone();
        let overlay_task = overlay_clone.clone();

        glib::spawn_future_local(async move {
            let game_task_clone = game_task.clone();
            let injector_task_clone = injector_task.clone();
            let result = tokio::task::spawn_blocking(move || injector_task_clone.backup_and_deploy(&game_task_clone)).await.unwrap();
            match result {
                Ok(_) => {
                    if let Some(instr) = injector_task.get_proton_instructions(&game_task) {
                        if let Some(display) = gtk4::gdk::Display::default() {
                            display.clipboard().set_text(&instr);
                        }
                        overlay_task.add_toast(Toast::new("Патч применён! Параметры запуска для Proton скопированы в буфер обмена."));
                    } else {
                        overlay_task.add_toast(Toast::new("Патч успешно применён!"));
                    }
                }
                Err(e) => {
                    overlay_task.add_toast(Toast::new(&format!("Ошибка: {}", e)));
                }
            }
        });
    });

    let game_clone_res = game.clone();
    let overlay_clone_res = toast_overlay.clone();
    let injector_clone_res = injector.clone();
    
    restore_button.connect_clicked(move |_| {
        let game_task = game_clone_res.clone();
        let injector_task = injector_clone_res.clone();
        let overlay_task = overlay_clone_res.clone();

        glib::spawn_future_local(async move {
            let result = tokio::task::spawn_blocking(move || injector_task.restore_original(&game_task)).await.unwrap();
            match result {
                Ok(_) => {
                    overlay_task.add_toast(Toast::new("Оригинальные файлы успешно восстановлены!"));
                }
                Err(e) => {
                    overlay_task.add_toast(Toast::new(&format!("Ошибка восстановления: {}", e)));
                }
            }
        });
    });

    row.add_suffix(&buttons_box);

    if !game.dlc_list.is_empty() {
        let dlc_info_row = ActionRow::builder()
            .title(format!("Найдено DLC: {}", game.dlc_list.len()))
            .subtitle(format!("IDs: {:?}", game.dlc_list))
            .build();
        row.add_row(&dlc_info_row);
    } else if game.category != AppCategory::SystemTool {
        let dlc_info_row = ActionRow::builder()
            .title("DLC не найдено или игра не поддерживает их")
            .build();
        row.add_row(&dlc_info_row);
    }

    row
}
