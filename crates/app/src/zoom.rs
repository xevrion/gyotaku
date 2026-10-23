//! Preview zoom keeps the image and its selectable text in one rectangle.
use gpui::{Bounds, Pixels, Point, point, px, size};

pub(crate) struct Zoom {
    pub scale: f32,
    pan: Point<Pixels>,
    start: f32,
    gesture_scale: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            scale: 1.,
            pan: point(px(0.), px(0.)),
            start: 1.,
            gesture_scale: 1.,
        }
    }
}

impl Zoom {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(&mut self) {
        self.start = self.scale;
        self.gesture_scale = 1.;
    }

    pub fn pinch(
        &mut self,
        delta: f32,
        position: Point<Pixels>,
        base: Bounds<Pixels>,
        area: Bounds<Pixels>,
    ) {
        if !delta.is_finite() {
            return;
        }
        self.gesture_scale = crate::platform::pinch_scale(self.gesture_scale, delta).max(0.01);
        self.set(self.start * self.gesture_scale, position, base, area);
    }

    pub fn set(
        &mut self,
        scale: f32,
        position: Point<Pixels>,
        base: Bounds<Pixels>,
        area: Bounds<Pixels>,
    ) {
        if !scale.is_finite()
            || !position.x.as_f32().is_finite()
            || !position.y.as_f32().is_finite()
        {
            return;
        }
        let scale = scale.clamp(1., 12.);
        let center = base.center() + self.pan;
        let ratio = scale / self.scale;
        self.pan += (position - center) * (1. - ratio);
        self.scale = scale;
        self.clamp(base, area);
    }

    pub fn move_by(&mut self, delta: Point<Pixels>, base: Bounds<Pixels>, area: Bounds<Pixels>) {
        self.pan += delta;
        self.clamp(base, area);
    }

    fn clamp(&mut self, base: Bounds<Pixels>, area: Bounds<Pixels>) {
        let x = ((base.size.width * self.scale - area.size.width).as_f32() / 2.).max(0.);
        let y = ((base.size.height * self.scale - area.size.height).as_f32() / 2.).max(0.);
        self.pan = point(
            px(self.pan.x.as_f32().clamp(-x, x)),
            px(self.pan.y.as_f32().clamp(-y, y)),
        );
    }

    pub fn rect(&mut self, base: Bounds<Pixels>, area: Bounds<Pixels>) -> Bounds<Pixels> {
        self.clamp(base, area);
        Bounds::centered_at(
            base.center() + self.pan,
            size(base.size.width * self.scale, base.size.height * self.scale),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn area() -> Bounds<Pixels> {
        Bounds::new(point(px(20.), px(40.)), size(px(1000.), px(500.)))
    }

    #[test]
    fn the_point_under_the_gesture_does_not_move() {
        let area = area();
        let mut zoom = Zoom::new();
        let focal = point(px(650.), px(300.));
        zoom.set(2., focal, area, area);
        let rect = zoom.rect(area, area);
        assert_eq!((focal - rect.origin) / 2., focal - area.origin);
    }

    #[test]
    fn zoom_and_pan_cannot_lose_the_image() {
        let area = area();
        let mut zoom = Zoom::new();
        zoom.set(100., area.center(), area, area);
        assert_eq!(zoom.scale, 12.);
        zoom.move_by(point(px(1e6), px(-1e6)), area, area);
        let rect = zoom.rect(area, area);
        assert!(rect.contains(&area.origin));
        assert!(rect.contains(&area.center()));
        zoom.set(0., area.center(), area, area);
        assert_eq!(zoom.rect(area, area), area);
    }

    #[test]
    fn invalid_events_do_not_corrupt_the_geometry() {
        let area = area();
        let mut zoom = Zoom::new();
        zoom.begin();
        zoom.pinch(f32::NAN, area.center(), area, area);
        zoom.set(f32::INFINITY, area.center(), area, area);
        assert_eq!(zoom.rect(area, area), area);
    }
}
