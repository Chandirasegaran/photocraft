use super::*;
use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::kittest::NodeT;
use egui_kittest::{Harness, kittest::Queryable};
use photocraft_doc::LayerId;

fn harness(kind: crate::theme::ThemeKind, ppp: f32) -> (Harness<'static, crate::PhotocraftApp>, LayerId, LayerId) {
    let mut s = photocraft_engine::Session::new();
    s.execute("file.new", json!({"width": 8, "height": 8})).unwrap();
    let red = LayerId(s.execute("layer.new.layer", json!({"name": "Red layer"})).unwrap()["layer"].as_u64().unwrap());
    s.execute("layer.setLabelColor", json!({"color": "red"})).unwrap();
    let other = LayerId(s.execute("layer.new.layer", json!({"name": "Other layer"})).unwrap()["layer"].as_u64().unwrap());
    let mut h = Harness::builder().with_size(vec2(1440.0, 900.0)).with_pixels_per_point(ppp).with_max_steps(64).build_eframe(move |cc| {
        crate::PhotocraftApp::setup_context(&cc.egui_ctx, kind);
        let mut app = crate::PhotocraftApp::new(s, crate::Services::default());
        app.ui.theme = kind;
        app
    });
    h.run_steps(8);
    (h, red, other)
}

fn click(h: &mut Harness<'_, crate::PhotocraftApp>, at: Pos2, button: PointerButton, modifiers: Modifiers) {
    h.event(Event::ModifiersChanged(modifiers));
    h.hover_at(at);
    h.step();
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button, pressed, modifiers });
        h.step();
    }
    h.run_steps(3);
}

fn row(h: &Harness<'_, crate::PhotocraftApp>, id: LayerId) -> egui::Rect {
    crate::layer_row_ui::recorded(&h.ctx).into_iter().find(|r| r.layer == id.0).unwrap().row
}

#[test]
fn layer_color_context_menu_edits_the_clicked_layer_and_preserves_multi_selection() {
    for ppp in [1.0, 2.0] {
        let (mut h, red, other) = harness(crate::theme::ThemeKind::Pro, ppp);
        let at = row(&h, red).center();
        click(&mut h, at, PointerButton::Secondary, Modifiers::NONE);
        let at = h.get_by_label("Color ⏵").rect().center();
        click(&mut h, at, PointerButton::Primary, Modifiers::NONE);
        let at = h.get_by_label("Seafoam").rect().center();
        click(&mut h, at, PointerButton::Primary, Modifiers::NONE);
        let st = h.state().session.active().unwrap();
        assert_eq!(st.active_layer, Some(red));
        assert_eq!(st.doc.layer(red).unwrap().label, LabelColor::Seafoam);
        assert_eq!(st.doc.layer(other).unwrap().label, LabelColor::None);
        h.state_mut().run("layer.select", json!({"layer": other.0, "mode": "add"})).unwrap();
        h.run_steps(4);
        let clicked = h.state().session.active().unwrap().doc.layer(red).unwrap();
        assert_eq!(common_color(h.state(), clicked, true), None);
        let at = row(&h, red).center();
        click(&mut h, at, PointerButton::Secondary, Modifiers::NONE);
        let at = h.get_by_label("Color ⏵").rect().center();
        click(&mut h, at, PointerButton::Primary, Modifiers::NONE);
        let at = h.get_by_label("Indigo").rect().center();
        click(&mut h, at, PointerButton::Primary, Modifiers::NONE);
        let st = h.state().session.active().unwrap();
        assert_eq!(st.selected_layers().len(), 2);
        assert_eq!(st.doc.layer(red).unwrap().label, LabelColor::Indigo);
        assert_eq!(st.doc.layer(other).unwrap().label, LabelColor::Indigo);
        assert_eq!(common_color(h.state(), st.doc.layer(red).unwrap(), true), Some(LabelColor::Indigo));
    }
}

#[test]
fn layer_color_background_is_confined_to_the_eye_column_in_every_theme() {
    fn rects(shape: &egui::Shape, color: egui::Color32, out: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(r) if r.fill == color => out.push(r.rect),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    rects(s, color, out);
                }
            }
            _ => {}
        }
    }
    /// The (texture, tint) of the icon painted exactly in `rect`.
    fn icon(shape: &egui::Shape, rect: egui::Rect) -> Option<(egui::TextureId, egui::Color32)> {
        match shape {
            egui::Shape::Rect(r) if r.rect == rect => r.brush.as_ref().map(|b| (b.fill_texture_id, r.fill)),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| icon(s, rect)),
            _ => None,
        }
    }
    fn eye_icon(h: &Harness<'_, crate::PhotocraftApp>, r: egui::Rect) -> Option<(egui::TextureId, egui::Color32)> {
        use egui::emath::GuiRounding;
        let cell = egui::Rect::from_center_size(pos2(r.left() + 15.0, r.center().y), vec2(15.0, 15.0)).round_to_pixels(h.ctx.pixels_per_point());
        h.output().shapes.iter().find_map(|s| icon(&s.shape, cell))
    }
    for kind in crate::theme::ThemeKind::ALL {
        let (mut h, red, other) = harness(kind, 1.0);
        let mut eyes = Vec::new();
        for visible in [true, false] {
            h.state_mut().run("layer.setProps", json!({"layer": red.0, "visible": visible})).unwrap();
            h.run_steps(4);
            let r = row(&h, red);
            let t = crate::theme::Tokens::for_kind(kind);
            let (_, bg) = t.layer_label_colors(LabelColor::Red).unwrap();
            let mut fills = Vec::new();
            for s in &h.output().shapes {
                rects(&s.shape, bg, &mut fills);
            }
            assert_eq!(fills, vec![egui::Rect::from_min_max(r.left_top(), pos2(r.left() + 30.0, r.bottom()))], "{kind:?}");
            eyes.push(eye_icon(&h, r).unwrap_or_else(|| panic!("centred eye in {kind:?} (visible: {visible})")));
        }
        // A hidden layer keeps a centred eye: the struck-out one, dimmer than the visible eye.
        let t = crate::theme::Tokens::for_kind(kind);
        let on_label = t.layer_label_icon(LabelColor::Red);
        assert_eq!((eyes[0].1, eyes[1].1), (on_label, on_label.gamma_multiply(0.6)), "eye tints on a colour label in {kind:?}");
        assert_ne!(eyes[0].0, eyes[1].0, "{kind:?}: a hidden layer shows the struck-out eye, not the eye");
        // Without a colour label the struck-out eye takes the theme's faint colour.
        h.state_mut().run("layer.setProps", json!({"layer": other.0, "visible": false})).unwrap();
        h.run_steps(4);
        let faint = crate::theme::Tokens::get(&h.ctx).text_faint;
        assert_eq!(eye_icon(&h, row(&h, other)), Some((eyes[1].0, faint)), "{kind:?}: unlabelled hidden layer");
        let eye = pos2(row(&h, red).left() + 17.0, row(&h, red).center().y);
        click(&mut h, eye, PointerButton::Primary, Modifiers::NONE);
        assert!(h.state().session.active().unwrap().doc.layer(red).unwrap().visible);
        assert_eq!(h.state().session.active().unwrap().doc.layer(red).unwrap().label, LabelColor::Red);
        click(&mut h, eye, PointerButton::Primary, Modifiers::ALT);
        assert!(h.state().session.active().unwrap().doc.layer(red).unwrap().visible);
    }
}

#[test]
fn layer_color_menu_marks_only_the_common_color() {
    let mut h = Harness::new_ui(|ui| {
        for c in LabelColor::ALL {
            color_button(ui, c, c == LabelColor::Blue);
        }
    });
    h.run();
    assert_eq!(h.get_by_label("Blue").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    assert_eq!(h.get_by_label("Red").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
}
