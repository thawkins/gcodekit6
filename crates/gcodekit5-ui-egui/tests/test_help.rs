//! The help browser: topics open by id and show their headings, lists, links, code and
//! images; internal links navigate with Back/Forward history, external links open in the
//! browser; the language follows the app language with an English fallback; every topic id
//! and image the markdown names is embedded; F1.

use std::collections::BTreeSet;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::resources;
use gcodekit5_ui_egui::windows::help::markdown::{Block, LinkTarget, Span, parse};
use gcodekit5_ui_egui::windows::help::{HelpTopic, HelpWindow, load_topic};

use gcodekit5_ui_egui::panes::cam_tools::CamTool;

use crate::machine_support::{app_harness, loopback_state};
use crate::test_cam_drill_press::open_tool;

/// A harness drawing only the help window, in English.
fn help_harness(topic: HelpTopic) -> Harness<'static, HelpWindow> {
    let mut window = HelpWindow::default();
    window.open(topic);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, window: &mut HelpWindow| window.show(ui.ctx(), &Translator::new("en")),
            window,
        );
    h.run_steps(3);
    h
}

/// Clicks the link (not the sidebar entry) labelled `label`.
fn click_link(h: &mut Harness<'static, HelpWindow>, label: &str) {
    h.get_by_role_and_label(Role::Link, label).click();
    h.run_steps(2);
}

/// Clicks the sidebar entry (not a link) labelled `label`.
fn click_sidebar(h: &mut Harness<'static, HelpWindow>, label: &str) {
    h.query_all_by_label(label)
        .find(|node| node.accesskit_node().role() != Role::Link)
        .expect("sidebar entry")
        .click();
    h.run_steps(2);
}

/// Every link and image a parsed topic names.
fn references(blocks: &[Block]) -> (Vec<LinkTarget>, Vec<String>) {
    let mut links = Vec::new();
    let mut images = Vec::new();
    for block in blocks {
        match block {
            Block::Heading { spans, .. }
            | Block::Paragraph(spans)
            | Block::ListItem { spans, .. } => {
                links.extend(spans.iter().filter_map(|span| match span {
                    Span::Link { target, .. } => Some(target.clone()),
                    Span::Text(_) | Span::Strong(_) | Span::Code(_) => None,
                }));
            }
            Block::Image { src, .. } => images.push(src.clone()),
            Block::Rule => {}
        }
    }
    (links, images)
}

#[test]
fn test_help_parses_headings_lists_links_code_and_images() {
    let md = "# Title\n\nSome **bold** and `code` text\ncontinued.\n\n## Sub\n\
              - [Designer](help:designer) item\n-     \u{25aa}\u{fe0f} [Gerber](help:gerber)\n\
              1. First\n<img src=\"../../help_images/order_objects_2\" alt=\"Order\" width=\"300\">\n\
              <ul>\n    <li>Position</li>\n</ul>\n---\nSee https://example.com/x.\n";
    let blocks = parse(md);
    assert_eq!(
        blocks[0],
        Block::Heading {
            level: 1,
            spans: vec![Span::Text("Title".into())]
        }
    );
    assert_eq!(
        blocks[1],
        Block::Paragraph(vec![
            Span::Text("Some ".into()),
            Span::Strong("bold".into()),
            Span::Text(" and ".into()),
            Span::Code("code".into()),
            Span::Text(" text continued.".into()),
        ])
    );
    assert!(matches!(&blocks[2], Block::Heading { level: 2, .. }));
    assert_eq!(
        blocks[3],
        Block::ListItem {
            depth: 0,
            marker: None,
            spans: vec![
                Span::Link {
                    text: "Designer".into(),
                    target: LinkTarget::Topic("designer".into())
                },
                Span::Text(" item".into()),
            ]
        }
    );
    assert_eq!(
        blocks[4],
        Block::ListItem {
            depth: 1,
            marker: None,
            spans: vec![Span::Link {
                text: "Gerber".into(),
                target: LinkTarget::Topic("gerber".into())
            }]
        }
    );
    assert_eq!(
        blocks[5],
        Block::ListItem {
            depth: 0,
            marker: Some("1.".into()),
            spans: vec![Span::Text("First".into())]
        }
    );
    assert_eq!(
        blocks[6],
        Block::Image {
            src: "order_objects_2.png".into(),
            alt: "Order".into(),
            width: 300.0
        }
    );
    assert_eq!(
        blocks[7],
        Block::ListItem {
            depth: 0,
            marker: None,
            spans: vec![Span::Text("Position".into())]
        }
    );
    assert_eq!(blocks[8], Block::Rule);
    assert_eq!(
        blocks[9],
        Block::Paragraph(vec![
            Span::Text("See ".into()),
            Span::Link {
                text: "https://example.com/x".into(),
                target: LinkTarget::External("https://example.com/x".into())
            },
            Span::Text(".".into()),
        ])
    );
    assert_eq!(blocks.len(), 10);
}

#[test]
fn test_help_opens_topic_by_id_and_renders_headings_and_images() {
    assert_eq!(HelpTopic::from_id("designer"), Some(HelpTopic::Designer));
    assert_eq!(HelpTopic::from_id("no_such_topic"), None);

    let h = help_harness(HelpTopic::Designer);
    assert_eq!(h.state().current(), HelpTopic::Designer);
    assert!(
        h.query_by_label("Main Actions in 2D Laser Mode").is_some(),
        "heading not shown"
    );
    assert!(
        h.query_by_label("Draw primitives (rectangles, circles, lines, ellipses, polylines, triangles, polygons, pinions, gears)")
            .is_some(),
        "list item not shown"
    );
    for alt in ["Designer", "Global Properties"] {
        assert!(
            h.query_all_by_role_and_label(Role::Image, alt)
                .next()
                .is_some(),
            "image {alt} not shown"
        );
    }
}

#[test]
fn test_help_internal_link_navigates_and_back_forward_return() {
    let mut h = help_harness(HelpTopic::Index);
    assert!(
        h.query_by_label("GCodeKit5 Help").is_some(),
        "index not shown"
    );
    click_link(&mut h, "Visualizer");
    assert_eq!(h.state().current(), HelpTopic::Visualizer);

    h.get_by_label("Back").click();
    h.run_steps(2);
    assert_eq!(h.state().current(), HelpTopic::Index);
    h.get_by_label("Forward").click();
    h.run_steps(2);
    assert_eq!(h.state().current(), HelpTopic::Visualizer);

    // A topic picked in the sidebar is history too, and drops what was forward.
    h.get_by_label("Back").click();
    h.run_steps(2);
    click_sidebar(&mut h, "CNC Tools");
    assert_eq!(h.state().current(), HelpTopic::ToolsManager);
    assert!(h.get_by_label("Forward").accesskit_node().is_disabled());
    h.get_by_label("Back").click();
    h.run_steps(2);
    assert_eq!(h.state().current(), HelpTopic::Index);
}

#[test]
fn test_help_external_link_opens_in_browser() {
    let mut window = HelpWindow::default();
    window.open(HelpTopic::Index);
    // The harness runs one frame per queued event, so the URLs are collected every frame.
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, (window, opened): &mut (HelpWindow, Vec<String>)| {
                window.show(ui.ctx(), &Translator::new("en"));
                ui.ctx().output(|o| {
                    opened.extend(o.commands.iter().filter_map(|c| match c {
                        egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
                        _ => None,
                    }));
                });
            },
            (window, Vec::new()),
        );
    h.run_steps(3);
    let url = "https://github.com/thawkins/gcodekit6";
    h.get_by_role_and_label(Role::Link, url).scroll_to_me();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Link, url).click();
    h.run_steps(2);
    assert_eq!(h.state().1, vec![url.to_string()]);
    assert_eq!(h.state().0.current(), HelpTopic::Index);
}

#[test]
fn test_help_language_follows_app_with_english_fallback() {
    let (lang, es) = load_topic(&Translator::new("es"), HelpTopic::Index);
    assert_eq!(lang, "es");
    assert!(es.contains("## Temas"));
    let (lang, en) = load_topic(&Translator::new("en"), HelpTopic::Index);
    assert_eq!(lang, "en");
    assert!(en.contains("## Topics"));
    // German has UI translations but no help pages: English is used.
    let (lang, de) = load_topic(&Translator::new("de"), HelpTopic::Index);
    assert_eq!(lang, "en");
    assert_eq!(de, en);
    assert_eq!(resources::help_markdown("de", "index"), None);
}

#[test]
fn test_help_every_topic_link_and_image_is_embedded() {
    for topic in HelpTopic::ALL {
        for lang in resources::HELP_LANGUAGES {
            assert!(
                resources::help_markdown(lang, topic.id()).is_some(),
                "{lang}/{}.md is not embedded",
                topic.id()
            );
        }
    }
    let mut embedded_images = BTreeSet::new();
    for (lang, topic, md) in resources::help_markdown_files() {
        assert!(
            HelpTopic::from_id(topic).is_some(),
            "{lang}/{topic}.md is not a help topic"
        );
        let (links, images) = references(&parse(md));
        for link in links {
            if let LinkTarget::Topic(id) = link {
                assert!(
                    HelpTopic::from_id(&id).is_some(),
                    "{lang}/{topic}.md links to missing topic {id:?}"
                );
            }
        }
        for image in images {
            assert!(
                resources::help_image(&image).is_some(),
                "{lang}/{topic}.md shows missing image {image:?}"
            );
            embedded_images.insert(image);
        }
    }
    assert!(!embedded_images.is_empty());
}

#[test]
fn test_help_embeds_every_resource_file() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
    let names = |dir: &str| -> BTreeSet<String> {
        std::fs::read_dir(root.join(dir))
            .expect("read resource dir")
            .map(|e| e.expect("entry"))
            // Subdirectories (icons/tabler) are embedded by their own module (icons.rs).
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    };
    for lang in resources::HELP_LANGUAGES {
        let embedded: BTreeSet<String> = resources::help_markdown_files()
            .filter(|(l, _, _)| *l == lang)
            .map(|(_, topic, _)| format!("{topic}.md"))
            .collect();
        assert_eq!(
            embedded,
            names(&format!("markdown/{lang}")),
            "markdown/{lang}"
        );
    }
    for (dir, embedded) in [
        ("help_images", resources::HELP_IMAGES),
        ("icons", resources::ICONS),
        ("images", resources::IMAGES),
    ] {
        let embedded: BTreeSet<String> = embedded.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(embedded, names(dir), "{dir}");
    }
    assert!(resources::icon("gear.svg").is_some());
    assert!(resources::image("eStop2.png").is_some());
}

#[test]
fn test_help_every_topic_renders_with_the_app_fonts() {
    // The harness panics on a character no installed font can draw (it would be a box).
    for lang in resources::HELP_LANGUAGES {
        let tr = Translator::new(lang);
        let mut window = HelpWindow::default();
        window.open(HelpTopic::Index);
        let mut h = Harness::builder()
            .with_size(egui::vec2(1200.0, 800.0))
            .build_ui_state(
                |ui, window: &mut HelpWindow| window.show(ui.ctx(), &tr),
                window,
            );
        h.run_steps(2);
        for topic in HelpTopic::ALL {
            h.state_mut().open(topic);
            h.run_steps(2);
            assert_eq!(h.state().current(), topic, "{lang}");
        }
    }
}

#[test]
fn test_help_f1_command_opens_index() {
    let (state, _shared) = loopback_state(std::time::Duration::from_millis(1));
    let mut h = app_harness(GcodeKitApp::from_state(state));
    assert!(!h.state().help().is_open());
    h.state_mut().dispatch(Command::OpenHelp);
    h.run_steps(3);
    assert!(h.state().help().is_open());
    assert_eq!(h.state().help().current(), HelpTopic::Index);
    assert!(
        h.query_by_label("GCodeKit5 Help").is_some(),
        "index not shown"
    );
}
/// The labels of every "Help (F1)" button on screen.
fn help_buttons(h: &Harness<'static, GcodeKitApp>) -> usize {
    h.query_all_by_label("Help (F1)").count()
}

/// Clicks the one help button on the page shown and returns the topic opened.
fn click_page_help(h: &mut Harness<'static, GcodeKitApp>, title: &str) -> HelpTopic {
    h.get_by_label("Help (F1)").click();
    h.run_steps(3);
    assert!(h.state().help().is_open(), "{title}: help not open");
    h.state().help().current()
}

#[test]
fn test_help_buttons_open_the_page_topic() {
    for (command, title, topic) in [
        (
            Command::OpenFirmwareSettings,
            "Device Config",
            HelpTopic::DeviceConfig,
        ),
        (
            Command::OpenDeviceManager,
            "Device Manager",
            HelpTopic::DeviceManager,
        ),
        (
            Command::OpenToolsManager,
            "CNC Tools",
            HelpTopic::ToolsManager,
        ),
        (
            Command::OpenMaterialsManager,
            "Materials",
            HelpTopic::MaterialsManager,
        ),
    ] {
        let (state, _shared) = loopback_state(std::time::Duration::from_millis(1));
        let mut h = app_harness(GcodeKitApp::from_state(state));
        h.state_mut().dispatch(command);
        h.run_steps(3);
        // The manager page replaces Machine Control (and its console's help button).
        assert_eq!(help_buttons(&h), 1, "{title}: no help button");
        assert_eq!(click_page_help(&mut h, title), topic, "{title}");
    }
}

#[test]
fn test_help_console_and_cam_pages_open_their_topic() {
    let (state, _shared) = loopback_state(std::time::Duration::from_millis(1));
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    // The console's help button (GTK's console header) opens Machine Control.
    h.get_by_label("Help (F1)").click();
    h.run_steps(3);
    assert_eq!(h.state().help().current(), HelpTopic::MachineControl);

    for tool in CamTool::ALL {
        let expected = match tool {
            CamTool::Jigsaw => Some("jigsaw_puzzle"),
            CamTool::TabbedBox => Some("tabbed_box_maker"),
            CamTool::BitmapEngraving => Some("laser_image_engraver"),
            CamTool::SpeedsFeeds => Some("speeds_feeds_calculator"),
            CamTool::Gerber => Some("gerber"),
            CamTool::SpoilboardGrid => Some("spoilboard_grid"),
            CamTool::SpoilboardSurfacing => Some("spoilboard_surfacing"),
            CamTool::DrillPress => Some("drill_press"),
            CamTool::ProbeRoutines => None,
        };
        assert_eq!(tool.help_topic().map(HelpTopic::id), expected, "{tool:?}");
    }
    let mut h = open_tool(CamTool::Gerber);
    let before = h.state().help().is_open();
    assert!(!before);
    // The CAM Tools page has no console: the Gerber page's is the only help button.
    let buttons: Vec<_> = h.query_all_by_label("Help (F1)").collect();
    assert_eq!(buttons.len(), 1, "Gerber page help button");
    buttons[0].click();
    h.run_steps(3);
    assert_eq!(h.state().help().current(), HelpTopic::Gerber);
}
