use image::{imageops, GenericImage, ImageBuffer, Pixel};

#[derive(Clone, Debug, Default)]
pub struct TextureBuilder<I: GenericImage> {
    images: Vec<I>,
}

impl<I: GenericImage> TextureBuilder<I>
where
    I::Pixel: 'static,
{
    pub fn new() -> Self {
        Self { images: Vec::new() }
    }

    pub fn overlay(&mut self, img: I) -> &mut Self {
        self.images.push(img);
        self
    }

    pub fn build(self) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>> {
        let mut base = ImageBuffer::new(self.images[0].width(), self.images[0].height());
        self.images
            .iter()
            .for_each(|img| imageops::overlay(&mut base, img, 0, 0));
        base
    }
}

#[derive(Clone, Debug)]
pub struct TextureArrayBuilder<I: GenericImage> {
    images: Vec<I>,
}

impl<I: GenericImage> TextureArrayBuilder<I> {
    pub fn new() -> Self {
        Self { images: Vec::new() }
    }

    pub fn push(&mut self, img: I) -> &mut Self {
        self.images.push(img);
        self
    }

    pub fn build(self) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>> {
        let mut base = ImageBuffer::new(
            self.images[0].width(),
            self.images.iter().map(|img| img.height()).sum(),
        );

        let mut h = 0;
        for img in self.images.iter() {
            base.copy_from(img, 0, h).unwrap();
            h += img.height();
        }

        base
    }
}

/// Generates texture arrays by superimposing textures from the listed folders, in order.
///
/// If a directory contains multiple images, the composite textures form a texture array.
///
/// If the number of images in a folder does not match the number of indices provided, the image list will repeat to fill the remaining indices.
///
/// ```no_run
/// /*
/// assets/textures/
///     eyes/
///         0.png
///         1.png
///     mouth/
///         0.png
///         1.png
///         2.png
///         3.png
/// */
///
/// use image::ImageBuffer;
/// let tex: ImageBuffer<_, _> = texture_array![3, "eyes", "mouth"];
///
/// // Images created:
/// // [1] = [eyes/0, mouth/0]
/// // [2] = [eyes/1, mouth/1]
/// // [3] = [eyes/0, mouth/2]
/// ```
#[macro_export]
macro_rules! texture_array {
    [$indices:expr, $( $path:literal ),* $(,)?] => {
        {
            use std::path::Path;
            use grin_asset::texture::{TextureArrayBuilder, TextureBuilder};
            use image::{imageops, ImageBuffer, io::Reader as ImageReader};

            let mut raw_textures: Vec<[ImageBuffer<_, _>; $indices]> = Vec::new();
            $(
                {
                    let mut p = Path::new("assets/textures/components/").to_path_buf();
                    p.push($path);

                    let mut vec = p
                        .read_dir().unwrap()
                        .map(|f| {
                            ImageReader::open(f.unwrap().path()).unwrap()
                                .decode().unwrap()
                                .into_rgba8()
                        })
                        .collect::<Vec<_>>();

                    let og_len = vec.len();
                    while vec.len() < $indices {
                        vec.extend_from_within(0..og_len);
                    }
                    vec.truncate($indices);

                    raw_textures.push(
                        vec
                            .try_into()
                            .expect("The number of component images does not match the length of the texture array.")
                    );
                }
            )*

            let mut tex_array_builder = TextureArrayBuilder::new();
            for i in 0..$indices {
                let mut tex_builder = TextureBuilder::new();
                for tex_list in raw_textures.iter() {
                    tex_builder.overlay(tex_list[i].clone());
                }
                tex_array_builder.push(tex_builder.build());
            }
            imageops::flip_horizontal(&tex_array_builder.build())
        }
    };
}
