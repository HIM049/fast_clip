#[derive(Debug)]
pub struct PlayerSize {
    orignal_size: (u32, u32),
    view_size: (u32, u32),
    output_size: (u32, u32),
}

impl PlayerSize {
    pub fn new() -> Self {
        Self {
            orignal_size: (1, 1),
            view_size: (0, 0),
            output_size: (1, 1),
        }
    }
    pub fn set_size(&mut self, orignal: Option<(u32, u32)>, view: Option<(u32, u32)>) {
        if let Some(o) = orignal {
            self.orignal_size = o;
        }
        if let Some(v) = view {
            self.view_size = v;
        }
        self.output_size = calc_output_size(self.orignal_size, self.view_size);
    }

    pub fn set_orignal(&mut self, size: (u32, u32)) {
        self.set_size(Some(size), None);
    }
    pub fn set_view(&mut self, size: (u32, u32)) {
        self.set_size(None, Some(size));
    }

    pub fn orignal_size(&self) -> (u32, u32) {
        self.orignal_size
    }
    pub fn view_size(&self) -> (u32, u32) {
        self.view_size
    }
    pub fn output_size(&self) -> (u32, u32) {
        self.output_size
    }
}

fn calc_output_size(orignal_size: (u32, u32), view_size: (u32, u32)) -> (u32, u32) {
    if orignal_size == (0, 0)
        || orignal_size == (1, 1)
        || view_size == (0, 0)
        || view_size == (1, 1)
    {
        return (1, 1);
    }

    let orignal_width = orignal_size.0;
    let orignal_height = orignal_size.1;
    let view_width = view_size.0;
    let view_height = view_size.1;

    let scale_w = view_width as f32 / orignal_width as f32;
    let scale_h = view_height as f32 / orignal_height as f32;
    let scale = scale_w.min(scale_h);

    let out_width = (orignal_width as f32 * scale).round() as u32;
    let out_height = (orignal_height as f32 * scale).round() as u32;

    (out_width, out_height)
}
