use crate::models::ChartDataPoint;
use crate::types::SvgConfig;
use alloy::primitives::U256;
use svg::node::element::{Group, Line, Path, Rectangle, Text};
use svg::Document;

/// Generate an SVG line chart from chart data points
pub fn generate_svg(data_points: &[ChartDataPoint], config: &SvgConfig) -> String {
    if data_points.is_empty() {
        return generate_empty_svg(config);
    }

    let chart_width = config.width - (2 * config.padding);
    let chart_height = config.height - (2 * config.padding);

    // Find min/max prices for scaling
    let (min_price, max_price) = find_price_range(data_points);
    let price_range = if max_price > min_price {
        max_price - min_price
    } else {
        U256::from(1) // Avoid division by zero for constant values
    };

    // Build SVG document
    let mut document = Document::new()
        .set("viewBox", (0, 0, config.width, config.height))
        .set("width", config.width)
        .set("height", config.height)
        .set("xmlns", "http://www.w3.org/2000/svg");

    // Add background if specified
    if let Some(ref bg_color) = config.background_color {
        let background = Rectangle::new()
            .set("x", 0)
            .set("y", 0)
            .set("width", config.width)
            .set("height", config.height)
            .set("fill", bg_color.as_str());
        document = document.add(background);
    }

    // Create chart group with padding offset
    let chart_group = Group::new()
        .set("transform", format!("translate({}, {})", config.padding, config.padding));

    // Add grid lines (optional, for better visualization)
    let grid_group = create_grid(chart_width, chart_height);

    // Build the path for the line chart
    let path_data = build_path_data(
        data_points,
        chart_width as f64,
        chart_height as f64,
        &min_price,
        &price_range,
    );

    let path = Path::new()
        .set("d", path_data)
        .set("fill", "none")
        .set("stroke", config.line_color.as_str())
        .set("stroke-width", config.line_width)
        .set("stroke-linecap", "round")
        .set("stroke-linejoin", "round");

    document = document
        .add(chart_group.add(grid_group).add(path));

    document.to_string()
}

/// Generate an empty SVG with a message
fn generate_empty_svg(config: &SvgConfig) -> String {
    let mut document = Document::new()
        .set("viewBox", (0, 0, config.width, config.height))
        .set("width", config.width)
        .set("height", config.height)
        .set("xmlns", "http://www.w3.org/2000/svg");

    // Add background if specified
    if let Some(ref bg_color) = config.background_color {
        let background = Rectangle::new()
            .set("x", 0)
            .set("y", 0)
            .set("width", config.width)
            .set("height", config.height)
            .set("fill", bg_color.as_str());
        document = document.add(background);
    }

    // Add "No Data" text
    let text = Text::new("No data available")
        .set("x", config.width / 2)
        .set("y", config.height / 2)
        .set("text-anchor", "middle")
        .set("dominant-baseline", "middle")
        .set("fill", "#666666")
        .set("font-family", "sans-serif")
        .set("font-size", "14");

    document = document.add(text);

    document.to_string()
}

/// Find the min and max price values in the data
fn find_price_range(data_points: &[ChartDataPoint]) -> (U256, U256) {
    let mut min = U256::MAX;
    let mut max = U256::ZERO;

    for point in data_points {
        let price = point.share_price.0;
        if price < min {
            min = price;
        }
        if price > max {
            max = price;
        }
    }

    // If all prices are the same, add some padding
    if min == max {
        let padding = min / U256::from(10);
        if padding > U256::ZERO {
            min = min.saturating_sub(padding);
            max = max.saturating_add(padding);
        } else {
            // Very small values, use fixed padding
            max = max.saturating_add(U256::from(1));
        }
    }

    (min, max)
}

/// Build the SVG path data string for the line chart
fn build_path_data(
    data_points: &[ChartDataPoint],
    chart_width: f64,
    chart_height: f64,
    min_price: &U256,
    price_range: &U256,
) -> String {
    let mut path_data = String::new();
    let point_count = data_points.len();

    if point_count == 0 {
        return path_data;
    }

    for (i, point) in data_points.iter().enumerate() {
        let x = if point_count > 1 {
            (i as f64 / (point_count - 1) as f64) * chart_width
        } else {
            chart_width / 2.0
        };

        let normalized_price = normalize_price(&point.share_price.0, min_price, price_range);
        let y = (1.0 - normalized_price) * chart_height; // Invert Y axis

        if i == 0 {
            path_data.push_str(&format!("M {} {}", x, y));
        } else {
            path_data.push_str(&format!(" L {} {}", x, y));
        }
    }

    path_data
}

/// Normalize a price value to a 0.0-1.0 range
fn normalize_price(price: &U256, min_price: &U256, price_range: &U256) -> f64 {
    if *price_range == U256::ZERO {
        return 0.5; // Middle of chart for constant values
    }

    let offset = price.saturating_sub(*min_price);

    // Convert to f64 for division (may lose precision for very large numbers)
    // This is acceptable for visualization purposes
    let offset_f64 = u256_to_f64(&offset);
    let range_f64 = u256_to_f64(price_range);

    if range_f64 > 0.0 {
        (offset_f64 / range_f64).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Convert U256 to f64 (may lose precision for very large numbers)
fn u256_to_f64(value: &U256) -> f64 {
    // For visualization, we can safely convert to f64
    // Large values will lose precision but will still render correctly
    let s = value.to_string();
    s.parse::<f64>().unwrap_or(0.0)
}

/// Create subtle grid lines for the chart
fn create_grid(width: u32, height: u32) -> Group {
    let mut grid = Group::new().set("stroke", "#e0e0e0").set("stroke-width", 0.5);

    // Horizontal grid lines (5 lines)
    for i in 0..=4 {
        let y = (height as f64 / 4.0) * i as f64;
        let line = Line::new()
            .set("x1", 0)
            .set("y1", y)
            .set("x2", width)
            .set("y2", y);
        grid = grid.add(line);
    }

    // Vertical grid lines (based on data points, max 10)
    for i in 0..=4 {
        let x = (width as f64 / 4.0) * i as f64;
        let line = Line::new()
            .set("x1", x)
            .set("y1", 0)
            .set("x2", x)
            .set("y2", height);
        grid = grid.add(line);
    }

    grid
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use models::types::U256Wrapper;

    #[test]
    fn test_generate_svg_with_data() {
        let data = vec![
            ChartDataPoint {
                timestamp: Utc::now(),
                share_price: U256Wrapper(U256::from(100)),
            },
            ChartDataPoint {
                timestamp: Utc::now(),
                share_price: U256Wrapper(U256::from(150)),
            },
            ChartDataPoint {
                timestamp: Utc::now(),
                share_price: U256Wrapper(U256::from(120)),
            },
        ];

        let config = SvgConfig::default();
        let svg = generate_svg(&data, &config);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<path"));
    }

    #[test]
    fn test_generate_empty_svg() {
        let config = SvgConfig::default();
        let svg = generate_svg(&[], &config);

        assert!(svg.contains("No data available"));
    }

    #[test]
    fn test_normalize_price() {
        let min = U256::from(100);
        let range = U256::from(100);

        let result = normalize_price(&U256::from(150), &min, &range);
        assert!((result - 0.5).abs() < 0.001);
    }
}
