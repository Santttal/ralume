//! AI-вкладка. Phase 19.c.2 — Processing queue + заглушка под «Coming soon».
//!
//! Источник данных — события `RecorderEvent::Transcription*`, которые уже
//! приходят в shell. shell вызывает `AiPage::handle_transcription_event(...)`
//! чтобы обновить in-memory список.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use libadwaita as adw;
use gtk4 as gtk;

use crate::transcription::queue::{QueueItem, QueueStatus};

pub struct AiPage {
    pub root: gtk::Widget,
    queue_list: gtk::ListBox,
    items: Rc<RefCell<Vec<QueueItem>>>,
}

impl AiPage {
    pub fn new() -> Rc<Self> {
        let paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .wide_handle(true)
            .hexpand(true)
            .vexpand(true)
            .build();

        // ----- Левая колонка: tools list -----
        let side = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .width_request(260)
            .build();
        let side_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(14)
            .margin_bottom(8)
            .margin_start(16)
            .margin_end(16)
            .build();
        let ic = gtk::Image::from_icon_name("starred-symbolic");
        side_header.append(&ic);
        let h = gtk::Label::new(Some("AI-инструменты"));
        h.add_css_class("heading");
        side_header.append(&h);
        side.append(&side_header);

        let tools_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(8)
            .margin_end(8)
            .build();
        tools_list.add_css_class("navigation-sidebar");
        let queue_row = make_tool_row(
            "alarm-symbolic",
            "Очередь задач",
            "Идущие и ожидающие транскрипции.",
        );
        tools_list.append(&queue_row);
        tools_list.select_row(Some(&queue_row));
        side.append(&tools_list);

        let coming_soon = gtk::Label::builder()
            .label("Скоро")
            .halign(gtk::Align::Start)
            .margin_top(14)
            .margin_bottom(6)
            .margin_start(14)
            .margin_end(14)
            .build();
        coming_soon.add_css_class("caption");
        coming_soon.add_css_class("dim-label");
        side.append(&coming_soon);

        for (icon, name, desc) in &[
            (
                "system-search-symbolic",
                "Поиск по библиотеке",
                "Искать слова и фразы во всех расшифровках.",
            ),
            (
                "text-x-generic-symbolic",
                "Авто-саммари",
                "Короткие сводки и разбиение на главы.",
            ),
        ] {
            let row = make_tool_row(icon, name, desc);
            row.set_sensitive(false);
            tools_list.append(&row);
        }

        paned.set_start_child(Some(&side));

        // ----- Правая колонка: queue view -----
        let main = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();
        let main_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .margin_top(18)
            .margin_bottom(12)
            .margin_start(20)
            .margin_end(20)
            .build();
        let title = gtk::Label::builder()
            .label("Очередь задач")
            .halign(gtk::Align::Start)
            .build();
        title.add_css_class("title-1");
        let subtitle = gtk::Label::builder()
            .label("Текущие и недавние задачи распознавания речи.")
            .halign(gtk::Align::Start)
            .build();
        subtitle.add_css_class("dim-label");
        main_header.append(&title);
        main_header.append(&subtitle);
        main.append(&main_header);

        let queue_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(16)
            .build();
        queue_list.add_css_class("boxed-list");
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&queue_list)
            .build();
        main.append(&scroll);
        paned.set_end_child(Some(&main));
        paned.set_position(260);

        let root: gtk::Widget = paned.upcast();

        let this = Rc::new(Self {
            root,
            queue_list,
            items: Rc::new(RefCell::new(Vec::new())),
        });
        this.rebuild();
        this
    }

    /// Занесите в очередь новую команду (вызывается shell, когда отправляет
    /// `UiCommand::TranscribeRequested`, до того как recorder ответит).
    pub fn enqueue(&self, path: PathBuf) {
        let mut items = self.items.borrow_mut();
        // Если уже есть в очереди (или обрабатывается) — не дублируем.
        if let Some(it) = items.iter_mut().find(|i| i.video_path == path) {
            it.status = QueueStatus::Queued;
            it.progress = 0.0;
        } else {
            items.push(QueueItem {
                video_path: path,
                status: QueueStatus::Queued,
                progress: 0.0,
            });
        }
        drop(items);
        self.rebuild();
    }

    pub fn mark_processing(&self, path: &Path, fraction: f64) {
        let mut items = self.items.borrow_mut();
        if let Some(it) = items.iter_mut().find(|i| i.video_path == path) {
            it.status = QueueStatus::Processing;
            it.progress = fraction.clamp(0.0, 1.0);
        } else {
            items.push(QueueItem {
                video_path: path.to_path_buf(),
                status: QueueStatus::Processing,
                progress: fraction.clamp(0.0, 1.0),
            });
        }
        drop(items);
        self.rebuild();
    }

    pub fn mark_done(&self, path: &Path) {
        let mut items = self.items.borrow_mut();
        if let Some(it) = items.iter_mut().find(|i| i.video_path == path) {
            it.status = QueueStatus::Done;
            it.progress = 1.0;
        }
        drop(items);
        self.rebuild();
    }

    pub fn mark_failed(&self, path: &Path, message: String) {
        let mut items = self.items.borrow_mut();
        if let Some(it) = items.iter_mut().find(|i| i.video_path == path) {
            it.status = QueueStatus::Failed(message);
        }
        drop(items);
        self.rebuild();
    }

    fn rebuild(&self) {
        while let Some(child) = self.queue_list.first_child() {
            self.queue_list.remove(&child);
        }
        let items = self.items.borrow();
        if items.is_empty() {
            let empty = gtk::Label::builder()
                .label("Очередь пуста.")
                .halign(gtk::Align::Center)
                .margin_top(30)
                .margin_bottom(30)
                .build();
            empty.add_css_class("dim-label");
            self.queue_list.append(&empty);
            return;
        }
        for it in items.iter() {
            self.queue_list.append(&make_queue_row(it));
        }
    }
}

fn make_tool_row(icon: &str, name: &str, desc: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let hbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    let ic = gtk::Image::from_icon_name(icon);
    ic.set_pixel_size(18);
    hbox.append(&ic);
    let vb = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(1)
        .build();
    let n = gtk::Label::builder()
        .label(name)
        .halign(gtk::Align::Start)
        .build();
    n.add_css_class("heading");
    let d = gtk::Label::builder()
        .label(desc)
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build();
    d.add_css_class("caption");
    d.add_css_class("dim-label");
    vb.append(&n);
    vb.append(&d);
    hbox.append(&vb);
    row.set_child(Some(&hbox));
    row
}

fn make_queue_row(item: &QueueItem) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let vb = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(14)
        .margin_end(14)
        .build();
    let title = gtk::Label::builder()
        .label(&item.title())
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    title.add_css_class("heading");
    vb.append(&title);

    let status = gtk::Label::builder()
        .label(&item.status_label())
        .halign(gtk::Align::Start)
        .build();
    status.add_css_class("caption");
    status.add_css_class("dim-label");
    vb.append(&status);

    if matches!(item.status, QueueStatus::Processing) {
        let bar = gtk::ProgressBar::builder().build();
        bar.set_fraction(item.progress);
        vb.append(&bar);
    }
    row.set_child(Some(&vb));
    row
}
