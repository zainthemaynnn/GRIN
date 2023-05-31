//! Dialogue window and text formatting.
//! Yes it's this short.

use bevy::prelude::*;
use html_parser::{Dom, Node};
use itertools::Itertools;

pub fn parse_dialogue(text: &str, default_style: &TextStyle) -> Text {
    let mut sections = Vec::new();
    let dom = Dom::parse(text).expect("malformed html dialogue");
    for n in dom.children.iter() {
        sections.append(&mut parse_section(n, default_style.clone()));
    }
    Text::from_sections(sections)
}

pub fn parse_color(color: Option<&str>) -> Result<Color, String> {
    // AVERAGE RUST PROGRAM
    let (r, g, b, a) = color
        .ok_or("no color specified")?
        .trim_matches(|c| c == '(' || c == ')')
        .split(", ")
        .into_iter()
        .map(|s| s.parse::<f32>().map_err(|e| e.to_string()))
        .collect_tuple()
        .ok_or("incorrect number of color components")?;
    Ok(Color::rgba(r?, g?, b?, a?))
}

pub fn parse_section(node: &Node, mut style: TextStyle) -> Vec<TextSection> {
    match node {
        Node::Text(t) => vec![TextSection::new(t, style)],
        Node::Element(e) => {
            for (attr, v) in e.attributes.iter() {
                match attr.as_str() {
                    "color" => style.color = parse_color(v.as_deref()).unwrap(),
                    "size" => {
                        style.font_size = v.as_deref().expect("no size specified").parse().unwrap()
                    }
                    _ => warn!("Unrecognized tag in dialogue: {}", attr),
                }
            }

            let mut sections = Vec::new();
            for n in e.children.iter() {
                sections.append(&mut parse_section(n, style.clone()));
            }
            sections
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test() {
        let text = dbg!(parse_dialogue(
            r#"
                just some white text, chilling
                <span color="(1.0, 0.0, 1.0, 1.0)">MY BROTHER IN TEXT, WE ONLY DO <span size=24>MAGENTA</span> OUT HERE</span>
                :(
            "#,
            &TextStyle::default(),
        ));
        assert_eq!(text.sections.len(), 5);
        assert_eq!(text.sections[0].style.color, Color::WHITE);
        assert_eq!(text.sections[1].style.color, Color::FUCHSIA);
        assert_eq!(text.sections[2].style.color, Color::FUCHSIA);
        assert_eq!(text.sections[3].style.color, Color::FUCHSIA);
        assert_eq!(text.sections[4].style.color, Color::WHITE);
        assert_eq!(text.sections[1].style.font_size, 12.0);
        assert_eq!(text.sections[2].style.font_size, 24.0);
        assert_eq!(text.sections[3].style.font_size, 12.0);
    }
}
