use std::sync::Arc;

use bytes::Bytes;
use druid::{
    widget::{Controller, Image, Label, SizedBox, ViewSwitcher},
    Data, ImageBuf, Lens, Selector, Target, Widget, WidgetExt, WidgetId,
};

#[derive(Clone, Data, Lens)]
pub struct NetImageState {
    pub image_ref: ImageRef,
    pub data: Option<Arc<Bytes>>,
}

impl NetImageState {
    pub fn cover(id: &str) -> Self {
        NetImageState {
            image_ref: ImageRef::Cover(id.to_owned()),
            data: None,
        }
    }
}

#[derive(Clone, Data, Debug, Eq, PartialEq)]
pub enum ImageRef {
    Cover(String),
    #[allow(unused)]
    Url(String),
}

pub struct ImageLoadRequest {
    pub image_ref: ImageRef,
    pub target_widget: WidgetId,
}

pub struct ImageLoadFinished {
    pub image_ref: ImageRef,
    pub data: Arc<Bytes>,
}

pub const REQUEST_IMAGE_LOAD: Selector<ImageLoadRequest> =
    Selector::new("tinysonic.net_image.request");
pub const IMAGE_LOADED: Selector<ImageLoadFinished> = Selector::new("tinysonic.net_image.loaded");

struct RequestAndUpdateImage;
impl<W: Widget<NetImageState>> Controller<NetImageState, W> for RequestAndUpdateImage {
    fn lifecycle(
        &mut self,
        child: &mut W,
        ctx: &mut druid::LifeCycleCtx,
        event: &druid::LifeCycle,
        data: &NetImageState,
        env: &druid::Env,
    ) {
        if let druid::LifeCycle::WidgetAdded = event {
            println!(
                "requesting initial image from {:?} for {:?}",
                data.image_ref,
                ctx.widget_id()
            );

            ctx.submit_command(
                REQUEST_IMAGE_LOAD
                    .with(ImageLoadRequest {
                        image_ref: data.image_ref.clone(),
                        target_widget: ctx.widget_id(),
                    })
                    .to(Target::Auto),
            );
        }
        child.lifecycle(ctx, event, data, env)
    }

    fn update(
        &mut self,
        child: &mut W,
        ctx: &mut druid::UpdateCtx,
        old_data: &NetImageState,
        data: &NetImageState,
        env: &druid::Env,
    ) {
        if old_data.image_ref != data.image_ref {
            println!(
                "requesting image from {:?} for {:?}",
                data.image_ref,
                ctx.widget_id()
            );

            ctx.submit_command(
                REQUEST_IMAGE_LOAD
                    .with(ImageLoadRequest {
                        image_ref: data.image_ref.clone(),
                        target_widget: ctx.widget_id(),
                    })
                    .to(Target::Auto),
            );
        }
        child.update(ctx, old_data, data, env)
    }

    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut druid::EventCtx,
        event: &druid::Event,
        data: &mut NetImageState,
        env: &druid::Env,
    ) {
        if let druid::Event::Command(cmd) = event {
            if let Some(loaded) = cmd.get(IMAGE_LOADED) {
                if data.image_ref == loaded.image_ref {
                    data.data = Some(loaded.data.clone());
                } else {
                    println!("wrong image ref!");
                }
            }
        }

        child.event(ctx, event, data, env)
    }
}

pub fn net_image() -> impl Widget<NetImageState> {
    SizedBox::new(ViewSwitcher::new(
        |s: &NetImageState, _env| s.data.is_some(),
        |has_data, s, _env| {
            if *has_data {
                let decoded = ImageBuf::from_data(s.data.as_ref().unwrap());
                match decoded {
                    Ok(data) => Box::new(Image::new(data)),
                    Err(e) => Box::new(Label::new(format!("{:?}", e))),
                }
            } else {
                Box::new(Label::new("...".to_string()))
            }
        },
    ))
    .width(300.0)
    .height(300.0)
    .controller(RequestAndUpdateImage)
}
