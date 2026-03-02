use super::CellRenderer;

impl CellRenderer {
    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Get the list of supported present modes for this surface
    pub fn supported_present_modes(&self) -> &[wgpu::PresentMode] {
        &self.supported_present_modes
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: par_term_config::VsyncMode) -> bool {
        self.supported_present_modes
            .contains(&mode.to_present_mode())
    }

    /// Update the vsync mode. Returns the actual mode applied (may differ if requested mode unsupported).
    /// Also returns whether the mode was changed.
    pub fn update_vsync_mode(
        &mut self,
        mode: par_term_config::VsyncMode,
    ) -> (par_term_config::VsyncMode, bool) {
        let requested = mode.to_present_mode();
        let current = self.config.present_mode;

        // Determine the actual mode to use
        let actual = if self.supported_present_modes.contains(&requested) {
            requested
        } else {
            log::warn!(
                "Requested present mode {:?} not supported, falling back to Fifo",
                requested
            );
            wgpu::PresentMode::Fifo
        };

        // Only reconfigure if the mode actually changed
        if actual != current {
            self.config.present_mode = actual;
            self.surface.configure(&self.device, &self.config);
            log::info!("VSync mode changed to {:?}", actual);
        }

        // Convert back to VsyncMode for return
        let actual_vsync = match actual {
            wgpu::PresentMode::Immediate => par_term_config::VsyncMode::Immediate,
            wgpu::PresentMode::Mailbox => par_term_config::VsyncMode::Mailbox,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => {
                par_term_config::VsyncMode::Fifo
            }
            _ => par_term_config::VsyncMode::Fifo,
        };

        (actual_vsync, actual != current)
    }

    /// Get the current vsync mode
    pub fn current_vsync_mode(&self) -> par_term_config::VsyncMode {
        match self.config.present_mode {
            wgpu::PresentMode::Immediate => par_term_config::VsyncMode::Immediate,
            wgpu::PresentMode::Mailbox => par_term_config::VsyncMode::Mailbox,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => {
                par_term_config::VsyncMode::Fifo
            }
            _ => par_term_config::VsyncMode::Fifo,
        }
    }
}
