//! Floating toolbar (phase 19.c.4) — borderless always-on-top окно с таймером,
//! Pause/Stop и переключателями mic/sys. Показывается на время записи.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use libadwaita as adw;
use gtk4 as gtk;

type VoidCallback = Rc<RefCell<Option<Box<dyn Fn()>>>>;

pub struct FloatingToolbar {
    window: gtk::Window,
    timer_label: gtk::Label,
    btn_pause: gtk::Button,
    btn_mic: gtk::ToggleButton,
    btn_sys: gtk::ToggleButton,
    on_pause_toggle: VoidCallback,
    on_stop: VoidCallback,
    on_mic_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_sys_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    paused: Rc<RefCell<bool>>,
}

impl FloatingToolbar {
    pub fn new(parent: &adw::ApplicationWindow) -> Rc<Self> {
        let window = gtk::Window::builder()
            .transient_for(parent)
            .decorated(false)
            .resizable(false)
            .modal(false)
            .title("Ralume — Запись")
            .build();
        window.set_default_size(1, 1);

        let hbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(10)
            .margin_end(10)
            .build();
        hbox.add_css_class("floating-toolbar");

        // Recording dot + timer.
        let dot = gtk::Label::new(Some("●"));
        dot.add_css_class("recording-dot");
        hbox.append(&dot);

        let timer_label = gtk::Label::new(Some("00:00:00"));
        timer_label.add_css_class("timer-label");
        hbox.append(&timer_label);

        hbox.append(&sep());

        let btn_pause = gtk::Button::builder()
            .icon_name("media-playback-pause-symbolic")
            .tooltip_text("Пауза")
            .build();
        btn_pause.add_css_class("flat");
        hbox.append(&btn_pause);

        let btn_stop = gtk::Button::builder()
            .icon_name("media-playback-stop-symbolic")
            .tooltip_text("Остановить")
            .build();
        btn_stop.add_css_class("destructive-action");
        hbox.append(&btn_stop);

        hbox.append(&sep());

        let btn_mic = gtk::ToggleButton::builder()
            .icon_name("audio-input-microphone-symbolic")
            .tooltip_text("Микрофон")
            .build();
        btn_mic.add_css_class("flat");
        hbox.append(&btn_mic);

        let btn_sys = gtk::ToggleButton::builder()
            .icon_name("audio-volume-high-symbolic")
            .tooltip_text("Звук системы")
            .build();
        btn_sys.add_css_class("flat");
        hbox.append(&btn_sys);

        window.set_child(Some(&hbox));

        let this = Rc::new(Self {
            window,
            timer_label,
            btn_pause,
            btn_mic,
            btn_sys,
            on_pause_toggle: Rc::new(RefCell::new(None)),
            on_stop: Rc::new(RefCell::new(None)),
            on_mic_toggle: Rc::new(RefCell::new(None)),
            on_sys_toggle: Rc::new(RefCell::new(None)),
            paused: Rc::new(RefCell::new(false)),
        });

        {
            let cb = this.on_pause_toggle.clone();
            this.btn_pause.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }
        {
            let cb = this.on_stop.clone();
            btn_stop.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }
        {
            let cb = this.on_mic_toggle.clone();
            this.btn_mic.connect_toggled(move |b| {
                if let Some(f) = cb.borrow().as_ref() {
                    f(b.is_active());
                }
            });
        }
        {
            let cb = this.on_sys_toggle.clone();
            this.btn_sys.connect_toggled(move |b| {
                if let Some(f) = cb.borrow().as_ref() {
                    f(b.is_active());
                }
            });
        }

        this
    }

    pub fn set_on_pause_toggle(&self, f: impl Fn() + 'static) {
        *self.on_pause_toggle.borrow_mut() = Some(Box::new(f));
    }
    pub fn set_on_stop(&self, f: impl Fn() + 'static) {
        *self.on_stop.borrow_mut() = Some(Box::new(f));
    }
    pub fn set_on_mic_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_mic_toggle.borrow_mut() = Some(Box::new(f));
    }
    pub fn set_on_sys_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_sys_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn show(&self, mic: bool, sys: bool) {
        self.btn_mic.set_active(mic);
        self.btn_sys.set_active(sys);
        self.window.present();
    }
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn update_timer(&self, secs: u64) {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        self.timer_label
            .set_label(&format!("{h:02}:{m:02}:{s:02}"));
    }

    pub fn set_paused(&self, paused: bool) {
        *self.paused.borrow_mut() = paused;
        self.btn_pause.set_icon_name(if paused {
            "media-playback-start-symbolic"
        } else {
            "media-playback-pause-symbolic"
        });
        self.btn_pause
            .set_tooltip_text(Some(if paused { "Продолжить" } else { "Пауза" }));
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }
}

fn sep() -> gtk::Separator {
    let s = gtk::Separator::new(gtk::Orientation::Vertical);
    s.set_margin_top(3);
    s.set_margin_bottom(3);
    s
}

// glib для сигналов timer — необходим, но больше ничего не делает.
#[allow(dead_code)]
fn _glib_used() -> Option<glib::SourceId> {
    None
}
