//! Terminal screenshot capture for MCP responses.
//!
//! Contains:
//! - `capture_terminal_screenshot_mcp_response`: capture renderer frame as PNG

use base64::Engine as _;
use par_term_mcp::TerminalScreenshotResponse;

use crate::app::window_state::WindowState;

impl WindowState {
    /// Capture a screenshot of the terminal and return it as an MCP response.
    pub(super) fn capture_terminal_screenshot_mcp_response(
        &mut self,
        request_id: &str,
    ) -> Result<TerminalScreenshotResponse, String> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| "No renderer available for screenshot".to_string())?;

        let image = renderer
            .take_screenshot()
            .map_err(|e| format!("Renderer screenshot failed: {e}"))?;
        let width = image.width();
        let height = image.height();

        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {e}"))?;
        let png_bytes = buf.into_inner();
        let data_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

        Ok(TerminalScreenshotResponse {
            request_id: request_id.to_string(),
            ok: true,
            error: None,
            mime_type: Some("image/png".to_string()),
            data_base64: Some(data_base64),
            width: Some(width),
            height: Some(height),
        })
    }
}
