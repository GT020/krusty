use iced::{
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text},
    Color, Element, Length, Point, Size,
};

/// A widget that can render various types of graphs (line, bar, etc.)
#[derive(Debug, Clone)]
pub struct GraphWidget {
    data: Vec<f32>,
    graph_type: GraphType,
    width: u32,
    height: u32,
    title: String,
    x_label: String,
    y_label: String,
    color: Color,
    background_color: Color,
    grid_color: Color,
    text_color: Color,
}

#[derive(Debug, Clone, Copy)]
pub enum GraphType {
    Line,
    Bar,
}

impl GraphWidget {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            graph_type: GraphType::Line,
            width: 400,
            height: 200,
            title: String::new(),
            x_label: String::new(),
            y_label: String::new(),
            color: Color::from_rgb(0.2, 0.6, 1.0), // Blue
            background_color: Color::WHITE,
            grid_color: Color::from_rgb(0.9, 0.9, 0.9),
            text_color: Color::BLACK,
        }
    }

    pub fn with_data<T: Into<f32> + Clone>(mut self, data: Vec<T>) -> Self {
        self.data = data.into_iter().map(|x| x.into()).collect();
        self
    }

    pub fn with_type(mut self, graph_type: GraphType) -> Self {
        self.graph_type = graph_type;
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_labels<S: Into<String>>(mut self, x_label: S, y_label: S) -> Self {
        self.x_label = x_label.into();
        self.y_label = y_label.into();
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn with_grid_color(mut self, color: Color) -> Self {
        self.grid_color = color;
        self
    }

    pub fn with_text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    pub fn view(self) -> Element<'static, canvas::Event> {
        let width = self.width;
        let height = self.height;
        let canvas = Canvas::new(self)
            .width(Length::Fixed(width as f32))
            .height(Length::Fixed(height as f32));
        canvas.into()
    }
}

impl canvas::Program<canvas::Event> for GraphWidget {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Clear background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.background_color);

        if self.data.is_empty() {
            return vec![frame.into_geometry()];
        }

        // Draw title
        if !self.title.is_empty() {
            let text = Text {
                content: self.title.clone(),
                position: Point::new(bounds.width / 2.0, 20.0),
                size: iced::Pixels(16.0),
                color: self.text_color,
                horizontal_alignment: iced::alignment::Horizontal::Center,
                ..Text::default()
            };
            frame.fill_text(text);
        }

        // Calculate drawing area (leave margin for labels)
        let margin = 40.0;
        let plot_width = bounds.width - 2.0 * margin;
        let plot_height = bounds.height - 2.0 * margin;
        let plot_origin = Point::new(margin, bounds.height - margin);

        // Draw axes
        let stroke = Stroke::default()
            .with_width(1.0)
            .with_color(self.text_color);
        frame.stroke(
            &Path::line(
                plot_origin,
                Point::new(plot_origin.x, plot_origin.y - plot_height),
            ),
            stroke,
        ); // Y-axis
        frame.stroke(
            &Path::line(
                plot_origin,
                Point::new(plot_origin.x + plot_width, plot_origin.y),
            ),
            stroke,
        ); // X-axis

        // Draw grid lines
        let grid_stroke = Stroke::default()
            .with_width(0.5)
            .with_color(self.grid_color);
        // Horizontal grid lines
        for i in 0..=5 {
            let y = plot_origin.y - (plot_height * i as f32 / 5.0);
            frame.stroke(
                &Path::line(
                    Point::new(plot_origin.x, y),
                    Point::new(plot_origin.x + plot_width, y),
                ),
                grid_stroke,
            );
        }
        // Vertical grid lines
        for i in 0..=5 {
            let x = plot_origin.x + (plot_width * i as f32 / 5.0);
            frame.stroke(
                &Path::line(
                    Point::new(x, plot_origin.y),
                    Point::new(x, plot_origin.y - plot_height),
                ),
                grid_stroke,
            );
        }

        // Draw labels (without rotation for simplicity)
        if !self.y_label.is_empty() {
            let text = Text {
                content: self.y_label.clone(),
                position: Point::new(10.0, bounds.height / 2.0),
                size: iced::Pixels(12.0),
                color: self.text_color,
                horizontal_alignment: iced::alignment::Horizontal::Center,
                ..Text::default()
            };
            frame.fill_text(text);
        }

        if !self.x_label.is_empty() {
            let text = Text {
                content: self.x_label.clone(),
                position: Point::new(bounds.width / 2.0, bounds.height - 10.0),
                size: iced::Pixels(12.0),
                color: self.text_color,
                horizontal_alignment: iced::alignment::Horizontal::Center,
                ..Text::default()
            };
            frame.fill_text(text);
        }

        // Normalize data for plotting
        let max_val = self.data.iter().cloned().fold(0.0f32, |a, b| a.max(b));
        let min_val = self
            .data
            .iter()
            .cloned()
            .fold(f32::INFINITY, |a, b| a.min(b));

        let range = if max_val == min_val {
            1.0
        } else {
            max_val - min_val
        };

        let normalized: Vec<f32> = self
            .data
            .iter()
            .map(|v| {
                let val = *v;
                (val - min_val) / range
            })
            .collect();

        // Draw graph based on type
        match self.graph_type {
            GraphType::Line => self.draw_line(
                &mut frame,
                &normalized,
                plot_origin,
                plot_width,
                plot_height,
            ),
            GraphType::Bar => self.draw_bars(
                &mut frame,
                &normalized,
                plot_origin,
                plot_width,
                plot_height,
            ),
        }

        vec![frame.into_geometry()]
    }
}

impl GraphWidget {
    fn draw_line(&self, frame: &mut Frame, data: &[f32], origin: Point, width: f32, height: f32) {
        if data.len() < 2 {
            return;
        }

        let mut path_builder = iced::widget::canvas::path::Builder::new();
        let point_width = width / (data.len() - 1) as f32;

        // Start point
        let x0 = origin.x;
        let y0 = origin.y - (data[0] * height);
        path_builder.move_to(Point::new(x0, y0));

        // Line segments
        for i in 1..data.len() {
            let x = origin.x + (i as f32 * point_width);
            let y = origin.y - (data[i] * height);
            path_builder.line_to(Point::new(x, y));
        }

        let path = path_builder.build();
        let stroke = Stroke::default().with_width(2.0).with_color(self.color);
        frame.stroke(&path, stroke);
    }

    fn draw_bars(&self, frame: &mut Frame, data: &[f32], origin: Point, width: f32, height: f32) {
        if data.is_empty() {
            return;
        }

        let bar_width = width / data.len() as f32 * 0.8; // Leave some space between bars
        let bar_spacing = width / data.len() as f32 * 0.2; // Space between bars

        for (i, &value) in data.iter().enumerate() {
            let x = origin.x + (i as f32 * width / data.len() as f32) + (bar_spacing / 2.0);
            let y = origin.y - (value * height);
            let bar_height = value * height;

            let rect = Path::rectangle(Point::new(x, y), Size::new(bar_width, bar_height));
            frame.fill(&rect, self.color);
        }
    }
}

/// Helper function to create a CPU usage graph widget
pub fn cpu_usage_graph_widget(cpu_data: Vec<f32>) -> GraphWidget {
    GraphWidget::new()
        .with_data(cpu_data)
        .with_type(GraphType::Line)
        .with_title("CPU Usage (%)")
        .with_labels("Time", "CPU %")
        .with_color(Color::from_rgb(0.2, 0.6, 1.0)) // Blue
        .with_size(400, 200)
}

/// Helper function to create a memory usage graph widget
pub fn memory_usage_graph_widget(memory_data: Vec<f32>) -> GraphWidget {
    GraphWidget::new()
        .with_data(memory_data)
        .with_type(GraphType::Line)
        .with_title("Memory Usage (MB)")
        .with_labels("Time", "Memory MB")
        .with_color(Color::from_rgb(0.2, 0.8, 0.2)) // Green
        .with_size(400, 200)
}
