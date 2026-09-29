use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolWindowMode {
    Embedded,
    Detached,
}

pub struct ToolWindowPolicy;

impl ToolWindowPolicy {
    pub fn mode(context: &egui::Context) -> ToolWindowMode {
        let embedded = context.input(|input| viewport_uses_embedded_tools(input.viewport()));
        if embedded {
            ToolWindowMode::Embedded
        } else {
            ToolWindowMode::Detached
        }
    }

    /// Restore and raise an existing tool surface without changing whether it is open.
    /// Detached viewports receive native focus; fullscreen/maximized fallback windows
    /// are moved to the top of the application's embedded window stack.
    pub fn raise(context: &egui::Context, viewport_id: egui::ViewportId, embedded_id: egui::Id) {
        match Self::mode(context) {
            ToolWindowMode::Embedded => {
                context.move_to_top(egui::LayerId::new(egui::Order::Middle, embedded_id));
            }
            ToolWindowMode::Detached => {
                context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Minimized(false));
                context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Visible(true));
                context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
            }
        }
    }
}

fn viewport_uses_embedded_tools(viewport: &egui::ViewportInfo) -> bool {
    if viewport.fullscreen.unwrap_or(false) || viewport.maximized.unwrap_or(false) {
        return true;
    }
    let Some(monitor) = viewport.monitor_size else {
        return false;
    };
    let Some(window) = viewport.outer_rect.or(viewport.inner_rect) else {
        return false;
    };
    // macOS "Zoom" can fill the usable screen without reporting the native
    // maximized flag. Size detection keeps tool windows inside that enlarged
    // workspace while leaving ordinary, merely large windows detached.
    window.width() >= monitor.x * 0.94 && window.height() >= monitor.y * 0.88
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolWindowSpec {
    pub default_size: [f32; 2],
    pub min_size: [f32; 2],
    pub resizable: bool,
    pub always_on_top: bool,
}

impl ToolWindowSpec {
    pub const fn new(default_size: [f32; 2], min_size: [f32; 2]) -> Self {
        Self {
            default_size,
            min_size,
            resizable: true,
            always_on_top: false,
        }
    }

    pub fn embedded(
        self,
        title: impl Into<egui::WidgetText>,
        id: egui::Id,
    ) -> egui::Window<'static> {
        egui::Window::new(title)
            .id(id)
            .resizable(self.resizable)
            .constrain(true)
            .default_size(self.default_size)
            .min_size(self.min_size)
    }

    pub fn viewport(self, title: impl Into<String>) -> egui::ViewportBuilder {
        let builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(self.default_size)
            .with_min_inner_size(self.min_size)
            .with_resizable(self.resizable);
        if self.always_on_top {
            builder.with_always_on_top()
        } else {
            builder
        }
    }
}

pub fn viewport_close_requested(context: &egui::Context) -> bool {
    context
        .input(|input| input.viewport().close_requested() || input.key_pressed(egui::Key::Escape))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode_for(fullscreen: bool, maximized: bool) -> ToolWindowMode {
        let context = egui::Context::default();
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.fullscreen = Some(fullscreen);
        viewport.maximized = Some(maximized);
        context.begin_pass(input);
        let mode = ToolWindowPolicy::mode(&context);
        let mut output = context.end_pass();
        output.textures_delta.clear();
        mode
    }

    fn mode_for_size(monitor: [f32; 2], window: [f32; 2]) -> ToolWindowMode {
        let context = egui::Context::default();
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.monitor_size = Some(egui::vec2(monitor[0], monitor[1]));
        viewport.outer_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(window[0], window[1]),
        ));
        viewport.fullscreen = Some(false);
        viewport.maximized = Some(false);
        context.begin_pass(input);
        let mode = ToolWindowPolicy::mode(&context);
        let mut output = context.end_pass();
        output.textures_delta.clear();
        mode
    }

    #[test]
    fn tool_windows_have_one_cross_platform_size_policy() {
        let spec = ToolWindowSpec::new([460.0, 360.0], [360.0, 140.0]);
        assert!(spec.resizable);
        assert!(!spec.always_on_top);
        assert!(spec.default_size[0] >= spec.min_size[0]);
        assert!(spec.default_size[1] >= spec.min_size[1]);
    }

    #[test]
    fn fullscreen_and_maximized_windows_embed_tools_without_changing_the_policy() {
        assert_eq!(mode_for(false, false), ToolWindowMode::Detached);
        assert_eq!(mode_for(true, false), ToolWindowMode::Embedded);
        assert_eq!(mode_for(false, true), ToolWindowMode::Embedded);
    }

    #[test]
    fn macos_zoom_sized_window_embeds_tools_without_affecting_normal_windows() {
        assert_eq!(
            mode_for_size([1512.0, 982.0], [1512.0, 900.0]),
            ToolWindowMode::Embedded
        );
        assert_eq!(
            mode_for_size([1512.0, 982.0], [1160.0, 812.0]),
            ToolWindowMode::Detached
        );
    }
}
