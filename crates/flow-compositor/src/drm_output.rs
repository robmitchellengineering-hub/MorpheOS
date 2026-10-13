//! Put the compositor's frame on a real display: one DRM "dumb buffer" on the first connected
//! output, written from the CPU.
//!
//! WHY A DUMB BUFFER AND NOT THE GPU. The renderer is already pixman (software), proven by
//! the headless tests, and a dumb buffer is the one display path every KMS driver supports:
//! QEMU's `virtio-gpu`, Intel, AMD, simpledrm on firmware framebuffers. It needs no Mesa,
//! no GBM and no EGL, so it cannot fail for the reasons GPU stacks fail on a first boot.
//! When it becomes a measured bottleneck, a GPU renderer replaces the renderer, not this
//! module's job of choosing an output and a mode. That is a later task with a measurement.
//!
//! The device comes from the seat session (live.rs), which hands over DRM master while
//! this session's virtual terminal is showing and takes it back when a person switches
//! away; [`Output::restore`] puts the picture back on their return.
//!
//! What it does not do yet: hotplug, more than one output, and page flipping (the frame is
//! copied into the one buffer being scanned out, so a fast-changing screen can tear).

use std::fs::File;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use drm::buffer::{Buffer, DrmFourcc};
use drm::control::{
    ClipRect, Device as ControlDevice, Mode, connector, crtc, dumbbuffer::DumbBuffer, framebuffer,
};

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl ControlDevice for Card {}

/// One output, showing one dumb buffer.
pub struct Output {
    card: Card,
    buffer: DumbBuffer,
    fb: framebuffer::Handle,
    crtc: crtc::Handle,
    connector: connector::Handle,
    mode: Mode,
    pub width: u32,
    pub height: u32,
}

impl Output {
    /// Take the opened device (`path` is for messages), pick the first connected connector
    /// and its preferred mode, and show a buffer of that size on it.
    pub fn open(fd: OwnedFd, path: &Path) -> Result<Self, String> {
        let card = Card(File::from(fd));
        let res = card
            .resource_handles()
            .map_err(|e| format!("reading {}'s resources: {e}", path.display()))?;
        let conn = res
            .connectors()
            .iter()
            .filter_map(|h| card.get_connector(*h, true).ok())
            .find(|c| c.state() == connector::State::Connected && !c.modes().is_empty())
            .ok_or_else(|| format!("{} has no connected display", path.display()))?;
        // The preferred mode comes first in the kernel's list.
        let mode: Mode = conn.modes()[0];
        let crtc = pick_crtc(&card, &res, &conn)?;
        let (w, h) = mode.size();
        let (width, height) = (u32::from(w), u32::from(h));
        let buffer = card
            .create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)
            .map_err(|e| format!("creating a {width}x{height} buffer: {e}"))?;
        let fb = card
            .add_framebuffer(&buffer, 24, 32)
            .map_err(|e| format!("adding the framebuffer: {e}"))?;
        card.set_crtc(crtc, Some(fb), (0, 0), &[conn.handle()], Some(mode))
            .map_err(|e| format!("setting the mode (is another process DRM master?): {e}"))?;
        eprintln!(
            "flow-compositor: showing on {} at {width}x{height}",
            path.display()
        );
        Ok(Self {
            card,
            buffer,
            fb,
            crtc,
            connector: conn.handle(),
            mode,
            width,
            height,
        })
    }

    /// Show the buffer again after the session comes back: while a person was on another
    /// virtual terminal, the console set its own mode.
    pub fn restore(&self) -> Result<(), String> {
        self.card
            .set_crtc(
                self.crtc,
                Some(self.fb),
                (0, 0),
                &[self.connector],
                Some(self.mode),
            )
            .map_err(|e| format!("restoring the mode: {e}"))
    }

    /// Copy a frame (ARGB, row-major, `width` x `height`) to the screen.
    pub fn show(&mut self, pixels: &[u32]) -> Result<(), String> {
        let pitch = self.buffer.pitch() as usize;
        let (w, h) = (self.width as usize, self.height as usize);
        let mut map = self
            .card
            .map_dumb_buffer(&mut self.buffer)
            .map_err(|e| format!("mapping the buffer: {e}"))?;
        for (y, row) in pixels.chunks_exact(w).take(h).enumerate() {
            let dst = &mut map[y * pitch..y * pitch + w * 4];
            for (d, p) in dst.as_chunks_mut::<4>().0.iter_mut().zip(row) {
                *d = p.to_le_bytes();
            }
        }
        drop(map);
        // Writing the buffer is not enough on drivers that copy it to the real scanout
        // (virtio-gpu, udl, simpledrm on some firmware): they show nothing until told what
        // changed. Drivers that scan the buffer out directly do not implement this call,
        // which is fine.
        let full = ClipRect::new(0, 0, clamp_u16(self.width), clamp_u16(self.height));
        match self.card.dirty_framebuffer(self.fb, &[full]) {
            // ENOSYS, which std reports as Unsupported.
            Err(e) if e.kind() != std::io::ErrorKind::Unsupported => {
                Err(format!("flushing the frame to the display: {e}"))
            }
            _ => Ok(()),
        }
    }
}

fn clamp_u16(v: u32) -> u16 {
    u16::try_from(v).unwrap_or(u16::MAX)
}

fn pick_crtc(
    card: &Card,
    res: &drm::control::ResourceHandles,
    conn: &connector::Info,
) -> Result<crtc::Handle, String> {
    // The CRTC the connector already drives, if any, then any CRTC its encoders can use.
    if let Some(crtc) = conn
        .current_encoder()
        .and_then(|e| card.get_encoder(e).ok())
        .and_then(|e| e.crtc())
    {
        return Ok(crtc);
    }
    conn.encoders()
        .iter()
        .filter_map(|e| card.get_encoder(*e).ok())
        .flat_map(|e| res.filter_crtcs(e.possible_crtcs()))
        .next()
        .ok_or_else(|| "no CRTC can drive the connected display".to_string())
}
