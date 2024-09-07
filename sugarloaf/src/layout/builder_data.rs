// Copyright (c) 2023-present, Raphael Amorim.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//
// layout_data.rs was originally retired from dfrg/swash_demo licensed under MIT
// https://github.com/dfrg/swash_demo/blob/master/LICENSE
//
// This file had updates to support color, underline_color, background_color
// and other functionalities

use crate::font_introspector::text::cluster::CharInfo;
use crate::font_introspector::Attributes;
use crate::font_introspector::Setting;
use crate::{sugarloaf::primitives::SugarCursor, Graphic};
use std::hash::{Hash, Hasher};

/// Data that describes a fragment.
#[derive(Copy, Debug, Clone)]
pub struct FragmentData {
    /// Identifier of the span that contains the fragment.
    pub span: usize,
    /// True if this fragment breaks shaping with the previous fragment.
    pub break_shaping: bool,
    /// Offset of the text.
    pub start: usize,
    /// End of the text.
    pub end: usize,
    /// Font variations.
    pub vars: FontSettingKey,
}

/// Data that describes an item.
#[derive(Copy, Debug, Clone)]
pub struct ItemData {
    /// Offset of the text.
    pub start: usize,
    /// End of the text.
    pub end: usize,
    /// Font variations.
    pub vars: FontSettingKey,
}

/// Builder Line State
#[derive(Default)]
pub struct BuilderLineText {
    /// Combined text.
    pub content: Vec<char>,
    /// Fragment index per character.
    pub frags: Vec<u32>,
    /// Span index per character.
    pub spans: Vec<usize>,
    /// Character info per character.
    pub info: Vec<CharInfo>,
    /// Offset of each character relative to its fragment.
    pub offsets: Vec<u32>,
}

#[derive(Default)]
pub struct BuilderLine {
    pub text: BuilderLineText,
    /// Collection of fragments.
    pub fragments: Vec<FragmentData>,
    /// Collection of items.
    pub items: Vec<ItemData>,
    /// Span index per character.
    pub styles: Vec<FragmentStyle>,
    /// Line Hash
    pub hash: u64,
}

/// Builder state.
#[derive(Default)]
pub struct BuilderState {
    /// Lines State
    pub lines: Vec<BuilderLine>,
    /// Font variation setting cache.
    pub vars: FontSettingCache<f32>,
    /// User specified scale.
    pub scale: f32,
    // Font size in ppem.
    pub font_size: f32,
}

impl BuilderState {
    /// Creates a new layout state.
    pub fn new() -> Self {
        let mut lines = vec![BuilderLine::default()];
        lines[0].styles.push(FragmentStyle::default());
        Self {
            lines,
            ..BuilderState::default()
        }
    }
    #[inline]
    pub fn new_line(&mut self) {
        self.lines.push(BuilderLine::default());
        let last = self.lines.len() - 1;
        self.lines[last].styles.push(FragmentStyle::default());
    }
    #[inline]
    pub fn current_line(&self) -> usize {
        let size = self.lines.len();
        if size == 0 {
            0
        } else {
            size - 1
        }
    }
    #[inline]
    pub fn clear(&mut self) {
        self.lines.clear();
        self.vars.clear();
    }

    #[inline]
    pub fn begin(&mut self) {
        self.lines.push(BuilderLine::default());
    }
}

/// Index into a font setting cache.
pub type FontSettingKey = u32;

/// Cache of tag/value pairs for font settings.
#[derive(Default)]
pub struct FontSettingCache<T: Copy + PartialOrd + PartialEq> {
    settings: Vec<Setting<T>>,
    lists: Vec<FontSettingList>,
    tmp: Vec<Setting<T>>,
}

impl<T: Copy + PartialOrd + PartialEq> FontSettingCache<T> {
    pub fn get(&self, key: u32) -> &[Setting<T>] {
        if key == !0 {
            &[]
        } else {
            self.lists
                .get(key as usize)
                .map(|list| list.get(&self.settings))
                .unwrap_or(&[])
        }
    }

    pub fn clear(&mut self) {
        self.settings.clear();
        self.lists.clear();
        self.tmp.clear();
    }
}

/// Sentinel for an empty set of font settings.
pub const EMPTY_FONT_SETTINGS: FontSettingKey = !0;

/// Range within a font setting cache.
#[derive(Copy, Clone)]
struct FontSettingList {
    pub start: u32,
    pub end: u32,
}

impl FontSettingList {
    pub fn get<T>(self, elements: &[T]) -> &[T] {
        elements
            .get(self.start as usize..self.end as usize)
            .unwrap_or(&[])
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum UnderlineShape {
    #[default]
    Regular = 0,
    Dotted = 1,
    Dashed = 2,
    Curly = 3,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct UnderlineInfo {
    pub offset: f32,
    pub size: f32,
    pub is_doubled: bool,
    pub shape: UnderlineShape,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FragmentStyleDecoration {
    // offset, size
    Underline(UnderlineInfo),
    Strikethrough,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct FragmentStyle {
    //  Unicode width
    pub width: f32,
    /// Font attributes.
    pub font_attrs: Attributes,
    /// Font color.
    pub color: [f32; 4],
    /// Background color.
    pub background_color: Option<[f32; 4]>,
    /// Font variations.
    pub font_vars: FontSettingKey,
    /// Additional spacing between letters (clusters) of text.
    // pub letter_spacing: f32,
    /// Additional spacing between words of text.
    // pub word_spacing: f32,
    /// Multiplicative line spacing factor.
    // pub line_spacing: f32,
    /// Enable underline decoration.
    pub decoration: Option<FragmentStyleDecoration>,
    /// Decoration color.
    pub decoration_color: Option<[f32; 4]>,
    /// Cursor style.
    pub cursor: Option<SugarCursor>,
    /// Media
    pub media: Option<Graphic>,
}

impl Default for FragmentStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            font_attrs: Attributes::default(),
            font_vars: EMPTY_FONT_SETTINGS,
            // letter_spacing: 0.,
            // word_spacing: 0.,
            // line_spacing: 1.,
            color: [1.0, 1.0, 1.0, 1.0],
            background_color: None,
            cursor: None,
            decoration: None,
            decoration_color: None,
            media: None,
        }
    }
}

impl Hash for FragmentStyle {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_attrs.hash(state);
        self.color[0].to_bits().hash(state);
        self.color[1].to_bits().hash(state);
        self.color[2].to_bits().hash(state);
        self.color[3].to_bits().hash(state);

        if let Some(bg_color) = self.background_color {
            bg_color[0].to_bits().hash(state);
            bg_color[1].to_bits().hash(state);
            bg_color[2].to_bits().hash(state);
            bg_color[3].to_bits().hash(state);
        }

        if let Some(color) = self.decoration_color {
            color[0].to_bits().hash(state);
            color[1].to_bits().hash(state);
            color[2].to_bits().hash(state);
            color[3].to_bits().hash(state);
        }

        match self.decoration {
            None => 0.hash(state),
            Some(FragmentStyleDecoration::Strikethrough) => 1.hash(state),
            Some(FragmentStyleDecoration::Underline(info)) => {
                match info.shape {
                    UnderlineShape::Regular => 2.hash(state),
                    UnderlineShape::Dotted => 3.hash(state),
                    UnderlineShape::Dashed => 4.hash(state),
                    UnderlineShape::Curly => 5.hash(state),
                }
                info.is_doubled.hash(state);
            }
        }
        match self.cursor {
            None => {
                0.hash(state);
            }
            Some(SugarCursor::Block(color)) => {
                1.hash(state);
                color[0].to_bits().hash(state);
                color[1].to_bits().hash(state);
                color[2].to_bits().hash(state);
                color[3].to_bits().hash(state);
            }
            Some(SugarCursor::Caret(color)) => {
                2.hash(state);
                color[0].to_bits().hash(state);
                color[1].to_bits().hash(state);
                color[2].to_bits().hash(state);
                color[3].to_bits().hash(state);
            }
            Some(SugarCursor::Underline(color)) => {
                3.hash(state);
                color[0].to_bits().hash(state);
                color[1].to_bits().hash(state);
                color[2].to_bits().hash(state);
                color[3].to_bits().hash(state);
            }
        };

        self.media.hash(state);
    }
}
