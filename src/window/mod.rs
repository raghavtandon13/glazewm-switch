use crate::{config::Config, komo::GlazeState, window::settings::Settings};
use winsafe::{prelude::*, *};

mod settings;

seq_ids! {
    ID_EXIT = 1001;
}

const TEXT_PADDING: i32 = 20;

pub struct Window {
    pub hwnd: HWND,
    state: GlazeState,
    settings: Settings,
    config: Config,
}

impl Window {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Self {
            hwnd: HWND::NULL,
            state: loop {
                match crate::komo::read_state() {
                    Ok(new_state) => break new_state,
                    Err(e) => {
                        log::error!("Failed to read state, retrying: {}", e);
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            },
            settings: Settings::new()?,
            config,
        })
    }

    pub fn register_class(&self, hinst: &HINSTANCE, class_name: &str) -> anyhow::Result<ATOM> {
        let mut wcx = WNDCLASSEX::default();
        wcx.lpfnWndProc = Some(Self::wnd_proc);
        wcx.hInstance = unsafe { hinst.raw_copy() };
        wcx.hCursor = HINSTANCE::NULL
            .LoadCursor(IdIdcStr::Idc(co::IDC::ARROW))?
            .leak();

        let mut wclass_name = if class_name.trim().is_empty() {
            WString::from_str(&format!(
                "WNDCLASS.{:#x}.{:#x}.{:#x}.{:#x}.{:#x}.{:#x}.{:#x}.{:#x}.{:#x}.{:#x}",
                wcx.style,
                wcx.lpfnWndProc.map_or(0, |p| p as usize),
                wcx.cbClsExtra,
                wcx.cbWndExtra,
                wcx.hInstance,
                wcx.hIcon,
                wcx.hCursor,
                wcx.hbrBackground,
                wcx.lpszMenuName(),
                wcx.hIconSm,
            ))
        } else {
            WString::from_str(class_name)
        };
        wcx.set_lpszClassName(Some(&mut wclass_name));

        SetLastError(co::ERROR::SUCCESS);
        match unsafe { RegisterClassEx(&wcx) } {
            Ok(atom) => Ok(atom),
            Err(err) => match err {
                co::ERROR::CLASS_ALREADY_EXISTS => {
                    let hinst = unsafe { wcx.hInstance.raw_copy() };
                    let (atom, _) = hinst.GetClassInfoEx(&wcx.lpszClassName().unwrap())?;
                    Ok(atom)
                }
                err => panic!("ERROR: Window::register_class: {}", err.to_string()),
            },
        }
    }

    pub fn create_window(
        &mut self,
        class_name: ATOM,
        pos: POINT,
        size: SIZE,
        hinst: &HINSTANCE,
    ) -> anyhow::Result<()> {
        if self.hwnd != HWND::NULL {
            panic!("Cannot create window twice.");
        }

        unsafe {
            HWND::CreateWindowEx(
                co::WS_EX::NOACTIVATE
                    | co::WS_EX::LAYERED
                    | co::WS_EX::TOOLWINDOW
                    | co::WS_EX::TOPMOST,
                AtomStr::Atom(class_name),
                None,
                co::WS::VISIBLE | co::WS::CLIPSIBLINGS | co::WS::POPUP,
                pos,
                size,
                None,
                IdMenu::None,
                hinst,
                Some(self as *const _ as _),
            )?
        };

        Ok(())
    }

    extern "system" fn wnd_proc(hwnd: HWND, msg: co::WM, wparam: usize, lparam: isize) -> isize {
        let wm_any = msg::WndMsg::new(msg, wparam, lparam);

        let ptr_self = match msg {
            co::WM::NCCREATE => {
                let msg = unsafe { msg::wm::NcCreate::from_generic_wm(wm_any) };
                let ptr_self = msg.createstruct.lpCreateParams as *mut Self;
                unsafe {
                    hwnd.SetWindowLongPtr(co::GWLP::USERDATA, ptr_self as _);
                }
                log::info!("HWND NCCREATE: {:#?}", hwnd);
                let ref_self = unsafe { &mut *ptr_self };
                ref_self.hwnd = unsafe { hwnd.raw_copy() };
                return unsafe { hwnd.DefWindowProc(wm_any) };
            }
            _ => hwnd.GetWindowLongPtr(co::GWLP::USERDATA) as *mut Self,
        };

        if ptr_self.is_null() {
            log::error!("Received message for uninitialized window: {:#?}", wm_any);
            return unsafe { hwnd.DefWindowProc(wm_any) };
        }

        let ref_self = unsafe { &mut *ptr_self };

        if msg == co::WM::NCDESTROY {
            log::info!("HWND NCDESTROY: {:#?}", hwnd);
            unsafe {
                ref_self.hwnd.SetWindowLongPtr(co::GWLP::USERDATA, 0);
            }
            ref_self.cleanup();
            return 0;
        }

        ref_self.handle_message(wm_any).unwrap_or_else(|err| {
            log::error!("Application error: {err}");
            0
        })
    }

    fn handle_message(&mut self, p: msg::WndMsg) -> anyhow::Result<isize> {
        match p.msg_id {
            co::WM::CREATE => self.handle_create(),
            co::WM::PAINT => self.handle_paint(),
            co::WM::LBUTTONDOWN => {
                self.handle_lbuttondown(unsafe { msg::wm::RButtonDown::from_generic_wm(p) })
            }
            co::WM::RBUTTONDOWN => {
                self.handle_rbuttondown(unsafe { msg::wm::RButtonDown::from_generic_wm(p) })
            }
            co::WM::COMMAND => self.handle_command(unsafe { msg::wm::Command::from_generic_wm(p) }),
            UpdateState::ID => self.handle_update_state(),
            SETTINGCHANGED => self.handle_setting_changed(),
            co::WM::DESTROY => {
                PostQuitMessage(0);
                Ok(0)
            }
            _ => Ok(unsafe { self.hwnd.DefWindowProc(p) }),
        }
    }

    fn handle_command(&mut self, mut p: msg::wm::Command) -> anyhow::Result<isize> {
        match p.event.ctrl_id() {
            ID_EXIT => {
                log::info!("Exiting application...");
                unsafe {
                    self.hwnd
                        .PostMessage(msg::WndMsg::new(co::WM::CLOSE, 0, 0))?;
                }
                Ok(0)
            }
            _ => Ok(unsafe { self.hwnd.DefWindowProc(p.as_generic_wm()) }),
        }
    }

    fn handle_rbuttondown(&mut self, p: msg::wm::RButtonDown) -> anyhow::Result<isize> {
        log::info!("Handling WM_RBUTTONDOWN message");
        let mut menu = HMENU::CreatePopupMenu()?;
        menu.append_item(&[winsafe::MenuItem::Entry {
            cmd_id: ID_EXIT,
            text: "Quit",
        }])?;

        menu.track_popup_menu_at_point(p.coords, &self.hwnd, &self.hwnd)?;
        menu.DestroyMenu()?;
        Ok(0)
    }

    fn handle_lbuttondown(&mut self, p: msg::wm::RButtonDown) -> anyhow::Result<isize> {
        log::info!("Handling WM_LBUTTONDOWN message");
        let hdc = self.hwnd.GetDC()?;
        let rect = self.hwnd.GetClientRect()?;

        let workspaces = self.workspaces()?;
        let focused_idx = workspaces.iter().position(|w| w.has_focus);

        let mut left = 0;
        for (idx, workspace) in workspaces.iter().enumerate() {
            let workspace_name = workspace
                .display_name
                .clone()
                .unwrap_or_else(|| workspace.name.clone().unwrap_or((idx + 1).to_string()));
            let sz = hdc.GetTextExtentPoint32(&workspace_name)?;

            let h_padding = if focused_idx == Some(idx) { 5 } else { 10 };
            let focused_rect = RECT {
                left: left + h_padding,
                right: left + sz.cx + TEXT_PADDING * 2 - h_padding,
                top: rect.bottom - 20,
                bottom: rect.bottom - 10,
            };

            if p.coords.x >= focused_rect.left && p.coords.x <= focused_rect.right {
                log::info!("Switching to workspace {}: {}", idx, workspace_name);
                crate::komo::focus_workspace(idx)?;
                break;
            }

            left += sz.cx + TEXT_PADDING * 2;
        }
        Ok(0)
    }

    fn handle_setting_changed(&mut self) -> anyhow::Result<isize> {
        log::info!("Handling WM_SETTINGCHANGE message");
        self.settings = Settings::new()?;
        self.hwnd.SetLayeredWindowAttributes(
            self.settings.colors.get_color_key(),
            0,
            co::LWA::COLORKEY,
        )?;
        self.resize_to_fit()?;
        self.hwnd.InvalidateRect(None, true)?;
        Ok(0)
    }

    fn workspaces(&self) -> anyhow::Result<&Vec<crate::komo::GlazeWorkspace>> {
        Ok(&self.state.workspaces)
    }

    fn paint_and_get_width(&self, hdc: &HDC, paint: bool) -> anyhow::Result<i32> {
        let _old_font = hdc.SelectObject(&self.settings.font)?;

        let rect = if paint {
            self.hwnd.GetClientRect()?
        } else {
            RECT::default()
        };

        let _old_pen = hdc.SelectObject(&self.settings.transparent_pen)?;

        if paint {
            hdc.FillRect(rect, &self.settings.transparent_brush)?;
            hdc.SetTextColor(self.settings.colors.foreground)?;
            hdc.SetBkMode(co::BKMODE::TRANSPARENT)?;
        }

        const BORDER_RADIUS: SIZE = SIZE { cx: 10, cy: 10 };

        let workspaces = self.workspaces()?;
        let focused_idx = workspaces.iter().position(|w| w.has_focus);

        match self.config.style {
            crate::config::Style::Windows => {
                self.paint_windows_style(hdc, paint, rect, BORDER_RADIUS, workspaces, focused_idx)
            }
            crate::config::Style::Classic => {
                self.paint_classic_style(hdc, paint, rect, BORDER_RADIUS, workspaces, focused_idx)
            }
        }
    }

    fn paint_windows_style(
        &self,
        hdc: &HDC,
        paint: bool,
        rect: RECT,
        border_radius: SIZE,
        workspaces: &Vec<crate::komo::GlazeWorkspace>,
        focused_idx: Option<usize>,
    ) -> anyhow::Result<i32> {
        let mut left = 0;
        for (idx, workspace) in workspaces.iter().enumerate() {
            let workspace_name = workspace
                .display_name
                .clone()
                .unwrap_or_else(|| workspace.name.clone().unwrap_or((idx + 1).to_string()));
            let sz = hdc.GetTextExtentPoint32(&workspace_name)?;

            let bg_color = if focused_idx == Some(idx) {
                self.settings.colors.focused
            } else if workspace.is_displayed {
                self.settings.colors.nonempty
            } else {
                self.settings.colors.empty
            };

            if paint {
                let box_rect = RECT {
                    left: left + 4,
                    right: left + sz.cx + TEXT_PADDING * 2 - 4,
                    top: 4,
                    bottom: rect.bottom - 4,
                };

                let bg_brush = HBRUSH::CreateSolidBrush(bg_color)?;
                let _old_brush = hdc.SelectObject(&*bg_brush);
                hdc.RoundRect(box_rect, border_radius)?;

                let text_rect = RECT {
                    left,
                    right: left + sz.cx + TEXT_PADDING * 2,
                    top: 0,
                    bottom: rect.bottom,
                };
                hdc.DrawText(
                    &workspace_name,
                    text_rect,
                    co::DT::CENTER | co::DT::VCENTER | co::DT::SINGLELINE,
                )?;
            }

            left += sz.cx + TEXT_PADDING * 2;
        }

        Ok(left)
    }

    fn paint_classic_style(
        &self,
        hdc: &HDC,
        paint: bool,
        rect: RECT,
        border_radius: SIZE,
        workspaces: &Vec<crate::komo::GlazeWorkspace>,
        focused_idx: Option<usize>,
    ) -> anyhow::Result<i32> {
        let mut left = 0;
        for (idx, workspace) in workspaces.iter().enumerate() {
            let workspace_name = workspace
                .display_name
                .clone()
                .unwrap_or_else(|| workspace.name.clone().unwrap_or((idx + 1).to_string()));
            let sz = hdc.GetTextExtentPoint32(&workspace_name)?;

            if paint {
                let text_rect = RECT {
                    left,
                    right: left + sz.cx + TEXT_PADDING * 2,
                    top: 0,
                    bottom: rect.bottom - 10,
                };
                hdc.DrawText(
                    &workspace_name,
                    text_rect,
                    co::DT::CENTER | co::DT::VCENTER | co::DT::SINGLELINE,
                )?;

                let h_padding = if focused_idx == Some(idx) { 5 } else { 10 };

                let focused_rect = RECT {
                    left: left + h_padding,
                    right: left + sz.cx + TEXT_PADDING * 2 - h_padding,
                    top: rect.bottom - 20,
                    bottom: rect.bottom - 10,
                };

                let focused_brush = HBRUSH::CreateSolidBrush(if focused_idx == Some(idx) {
                    self.settings.colors.focused
                } else if workspace.is_displayed {
                    self.settings.colors.nonempty
                } else {
                    self.settings.colors.empty
                })?;
                let _old_brush = hdc.SelectObject(&*focused_brush);
                hdc.RoundRect(focused_rect, border_radius)?;
            }

            left += sz.cx + TEXT_PADDING * 2;
        }

        Ok(left)
    }

    fn get_window_width(&self) -> anyhow::Result<i32> {
        let hdc = self.hwnd.GetDC()?;
        self.paint_and_get_width(&*hdc, false)
    }

    fn resize_to_fit(&self) -> anyhow::Result<bool> {
        let total_width = self.get_window_width()?;

        let rect = self.hwnd.GetClientRect()?;

        if rect.right - rect.left == total_width {
            return Ok(false);
        }

        self.hwnd.SetWindowPos(
            winsafe::HwndPlace::Place(co::HWND_PLACE::default()),
            POINT::default(),
            SIZE {
                cx: total_width,
                cy: rect.bottom - rect.top,
            },
            co::SWP::NOACTIVATE | co::SWP::NOZORDER | co::SWP::NOMOVE | co::SWP::NOREDRAW,
        )?;

        Ok(true)
    }

    fn handle_update_state(&mut self) -> anyhow::Result<isize> {
        self.state = loop {
            match crate::komo::read_state() {
                Ok(new_state) => break new_state,
                Err(e) => {
                    log::error!("Failed to read state on update: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        };
        self.resize_to_fit()?;
        self.hwnd.InvalidateRect(None, true)?;
        Ok(0)
    }

    fn handle_create(&self) -> anyhow::Result<isize> {
        log::info!("Handling WM_CREATE message");
        Ok(0)
    }

    fn handle_paint(&self) -> anyhow::Result<isize> {
        log::info!("Handling WM_PAINT message...");
        let hdc = self.hwnd.BeginPaint()?;
        self.paint_and_get_width(&*hdc, true)?;
        log::info!("WM_PAINT handled.");
        Ok(0)
    }

    fn cleanup(&mut self) {
        self.hwnd = HWND::NULL;
    }

    pub fn run_loop(&self) -> anyhow::Result<()> {
        let mut msg = MSG::default();
        while GetMessage(&mut msg, None, 0, 0)? {
            TranslateMessage(&msg);
            unsafe {
                DispatchMessage(&msg);
            }
        }
        Ok(())
    }

    pub fn prepare(&mut self) -> anyhow::Result<()> {
        if IsWindowsVistaOrGreater()? {
            SetProcessDPIAware()?;
        }

        let hinstance = HINSTANCE::GetModuleHandle(None)?;

        let atom = self.register_class(&hinstance, "komoswitch")?;

        let taskbar_atom = AtomStr::from_str("Shell_TrayWnd");
        let taskbar = HWND::FindWindow(Some(taskbar_atom), None)?
            .ok_or(anyhow::anyhow!("Taskbar not found"))?;

        let rect = taskbar.GetClientRect()?;

        let window_width = self.get_window_width()?;

        let x_pos = if self.config.position.x < 0 {
            let screen_width = unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                    windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
                )
            };
            (screen_width - window_width) / 2
        } else {
            self.config.position.x
        };

        self.create_window(
            atom,
            POINT {
                x: x_pos,
                y: self.config.position.y,
            },
            SIZE {
                cx: window_width,
                cy: rect.bottom - rect.top,
            },
            &hinstance,
        )?;

        self.hwnd.SetParent(&taskbar)?;

        self.hwnd.SetLayeredWindowAttributes(
            self.settings.colors.get_color_key(),
            0,
            co::LWA::COLORKEY,
        )?;

        Ok(())
    }
}

const SETTINGCHANGED: co::WM = unsafe { co::WM::from_raw(0x001A) };

pub struct UpdateState;

impl UpdateState {
    pub const ID: co::WM = unsafe { co::WM::from_raw(0x8000 + 1) };
}
