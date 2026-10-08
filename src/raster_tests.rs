use super::*;

fn raster(width: usize, height: usize, mode: u8) -> Vec<u8> {
    let stride = width / 8;
    let mut command = vec![GS, b'v', b'0', mode];
    command.extend_from_slice(&(stride as u16).to_le_bytes());
    command.extend_from_slice(&(height as u16).to_le_bytes());
    // Include control bytes to detect accidental interpretation of image data.
    command.extend((0..stride * height).map(|i| i as u8));
    command
}

fn column_image(width: usize, mode: u8) -> Vec<u8> {
    let mut command = vec![ESC, b'*', mode];
    command.extend_from_slice(&(width as u16).to_le_bytes());
    let bytes_per_column = if mode < 32 { 1 } else { 3 };
    command.extend(vec![0xff; width * bytes_per_column]);
    command
}

fn parse_chunks(chunks: &[&[u8]]) -> Vec<ReceiptElement> {
    let mut renderer = EscPosRenderer::new(false);
    let mut elements = Vec::new();
    for chunk in chunks {
        renderer.process_data(chunk).unwrap();
        // The TCP server drains the elements after every read.
        elements.extend(renderer.take_elements());
    }
    assert!(renderer.buffer.is_empty(), "unconsumed command bytes");
    assert!(
        renderer.current_line.is_empty(),
        "image data leaked into text"
    );
    elements
}

fn feed_dots(elements: &[ReceiptElement]) -> usize {
    elements
        .iter()
        .filter_map(|e| match e {
            ReceiptElement::PaperFeed { dots } => Some(dots),
            _ => None,
        })
        .sum()
}

#[test]
fn raster_slices_preserve_pixels_without_artificial_gaps() {
    let mut stream = vec![ESC, b'3', 0];
    let slice = raster(576, 24, 0);
    for _ in 0..3 {
        stream.extend_from_slice(&slice);
        stream.push(LF);
    }
    let elements = parse_chunks(&[&stream]);
    assert_eq!(elements.len(), 3);
    assert_eq!(feed_dots(&elements), 0);
    for element in elements {
        match element {
            ReceiptElement::RasterImage {
                width,
                height,
                data,
                bytes_per_line,
                scale,
                ..
            } => {
                assert_eq!(
                    (width, height, bytes_per_line, scale),
                    (576, 24, 72, [1, 1])
                );
                assert_eq!(data, slice[8..]);
            }
            other => panic!("unexpected element: {other:?}"),
        }
    }
}

#[test]
fn raster_commands_survive_every_tcp_split_position() {
    let mut stream = vec![ESC, b'@', ESC, b'3', 0, ESC, b'$', 7, 0, GS, b'W', 64, 2];
    stream.extend(raster(128, 24, 0));
    stream.push(LF);
    stream.extend(raster(576, 24, 0));
    stream.extend_from_slice(&[GS, b'V', 0]);
    let expected = parse_chunks(&[&stream]);
    assert_eq!(expected.len(), 3);
    for split in 0..=stream.len() {
        assert_eq!(
            parse_chunks(&[&stream[..split], &stream[split..]]),
            expected,
            "TCP split at byte {split}"
        );
    }
    let bytes: Vec<_> = stream.chunks(1).collect();
    assert_eq!(parse_chunks(&bytes), expected);
}

#[test]
fn column_images_feed_only_the_remainder_of_the_line() {
    for (spacing, expected_feed) in [(0, 0), (24, 0), (30, 6)] {
        let mut stream = vec![ESC, b'3', spacing];
        stream.extend(column_image(64, 33));
        stream.push(LF);
        stream.extend(column_image(64, 33));
        stream.push(LF);
        let expected = parse_chunks(&[&stream]);
        assert_eq!(feed_dots(&expected), expected_feed * 2);
        let bytes: Vec<_> = stream.chunks(1).collect();
        assert_eq!(parse_chunks(&bytes), expected);
    }
}

#[test]
fn explicit_paper_feed_uses_dots_and_current_line_spacing() {
    let mut stream = vec![ESC, b'3', 5];
    stream.extend(raster(32, 24, 0));
    stream.extend_from_slice(&[LF, ESC, b'd', 2, ESC, b'J', 12]);
    let expected = parse_chunks(&[&stream]);
    assert_eq!(feed_dots(&expected), 5 + 5 + 5 + 12);
    assert_eq!(
        parse_chunks(&stream.chunks(1).collect::<Vec<_>>()),
        expected
    );

    // Resetting the line spacing restores the default 30-dot blank line.
    assert_eq!(
        feed_dots(&parse_chunks(&[&[ESC, b'3', 0, ESC, b'2', LF]])),
        30
    );
}

#[test]
fn text_is_printed_before_feed_and_records_its_line_spacing() {
    let elements = parse_chunks(&[b"\x1b\x33\x28Hello\n"]);
    assert!(matches!(
        &elements[..],
        [ReceiptElement::Text { content, line_spacing: 40, .. }] if content == "Hello"
    ));
    let elements = parse_chunks(&[b"Hello\x1b\x4a\x07"]);
    assert!(matches!(
        &elements[..],
        [ReceiptElement::Text { content, line_spacing: 7, .. }] if content == "Hello"
    ));
    let elements = parse_chunks(&[b"Hello\x1b\x64\x00"]);
    assert!(matches!(
        &elements[..],
        [ReceiptElement::Text { content, line_spacing: 0, .. }] if content == "Hello"
    ));
}

#[test]
fn image_scaling_follows_protocol_modes() {
    for (mode, expected_scale) in [(0, [1, 1]), (1, [2, 1]), (2, [1, 2]), (3, [2, 2])] {
        for mode in [mode, mode + 48] {
            let elements = parse_chunks(&[&raster(32, 24, mode)]);
            assert!(matches!(
                &elements[..],
                [ReceiptElement::RasterImage { scale, .. }] if *scale == expected_scale
            ));
        }
    }
    for (mode, expected_scale) in [(0, [2, 3]), (1, [1, 3]), (32, [2, 1]), (33, [1, 1])] {
        let elements = parse_chunks(&[&column_image(32, mode)]);
        assert!(matches!(
            &elements[..],
            [ReceiptElement::RasterImage { scale, .. }] if *scale == expected_scale
        ));
    }
}

#[test]
fn rendered_slices_touch_and_keep_native_sizes_and_offsets() {
    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 800.0),
        )),
        ..Default::default()
    };
    let mut bounds = Vec::new();
    let output = context.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (width, height, offset, scale) in [
                (576, 24, 0, [1, 1]),
                (128, 24, 7, [1, 1]),
                (32, 24, 7, [2, 2]),
            ] {
                let top = ui.cursor().top();
                render_raster_image(
                    ui,
                    width,
                    height,
                    &vec![0xff; width / 8 * height],
                    offset,
                    4,
                    &Alignment::Left,
                    576.0,
                    width / 8,
                    0,
                    scale,
                );
                bounds.push((top, ui.cursor().top()));
            }
        });
    });
    assert_eq!(bounds[0].1 - bounds[0].0, 24.0);
    assert_eq!(bounds[1].1 - bounds[1].0, 24.0);
    assert_eq!(bounds[2].1 - bounds[2].0, 48.0);
    assert_eq!(bounds[0].1, bounds[1].0);
    assert_eq!(bounds[1].1, bounds[2].0);
    let images: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 3);
    assert_eq!(images[0].width(), 576.0);
    assert_eq!(images[1].width(), 128.0);
    assert_eq!(images[2].width(), 64.0);
    assert_eq!(images[1].left() - images[0].left(), 7.0);
    assert_eq!(images[2].left() - images[0].left(), 7.0);
}

#[tokio::test]
async fn tcp_receipt_preserves_bitmap_slices() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let state = AppState::new();
    let server_state = state.clone();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, peer) = listener.accept().await.unwrap();
        handle_client(socket, peer, server_state, false)
            .await
            .unwrap();
    });
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut receipt = vec![ESC, b'3', 0];
    receipt.extend(raster(576, 24, 0));
    receipt.push(LF);
    receipt.extend(raster(128, 24, 0));
    receipt.push(LF);
    for chunk in receipt.chunks(13) {
        socket.write_all(chunk).await.unwrap();
        tokio::task::yield_now().await;
    }
    socket.shutdown().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*state.elements.lock().unwrap(), parse_chunks(&[&receipt]));
}
