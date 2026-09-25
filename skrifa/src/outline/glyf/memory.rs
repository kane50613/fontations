//! Memory allocation for TrueType scaling.

use read_fonts::{
    mem::take_slice,
    tables::glyf::PointFlags,
    types::{F26Dot6, Fixed, Point},
};

use super::{super::Hinting, Outline};

/// Buffers used during HarfBuzz-style glyph scaling.
#[cfg(feature = "harfbuzz_path_style")]
pub(crate) struct HarfBuzzOutlineMemory<'a> {
    pub points: &'a mut [Point<f32>],
    pub contours: &'a mut [u16],
    pub flags: &'a mut [PointFlags],
    pub deltas: &'a mut [Point<f32>],
    pub iup_buffer: &'a mut [Point<f32>],
    pub composite_deltas: &'a mut [Point<f32>],
}

#[cfg(feature = "harfbuzz_path_style")]
impl<'a> HarfBuzzOutlineMemory<'a> {
    pub(super) fn new(outline: &Outline, buf: &'a mut [u8]) -> Option<Self> {
        let (points, buf) = take_slice(buf, outline.points)?;
        let (contours, buf) = take_slice(buf, outline.contours)?;
        let (flags, buf) = take_slice(buf, outline.points)?;
        // Don't allocate any delta buffers if we don't have variations
        let (deltas, iup_buffer, composite_deltas, _buf) = if outline.has_variations {
            let (deltas, buf) = take_slice(buf, outline.max_simple_points)?;
            let (iup_buffer, buf) = take_slice(buf, outline.max_simple_points)?;
            let (composite_deltas, buf) = take_slice(buf, outline.max_component_delta_stack)?;
            (deltas, iup_buffer, composite_deltas, buf)
        } else {
            (
                Default::default(),
                Default::default(),
                Default::default(),
                buf,
            )
        };
        Some(Self {
            points,
            contours,
            flags,
            deltas,
            iup_buffer,
            composite_deltas,
        })
    }
}

/// Buffers used during glyph scaling.
pub(crate) struct FreeTypeOutlineMemory<'a> {
    pub unscaled: &'a mut [Point<i32>],
    pub scaled: &'a mut [Point<F26Dot6>],
    #[cfg(feature = "hinting")]
    pub original_scaled: &'a mut [Point<F26Dot6>],
    pub contours: &'a mut [u16],
    pub flags: &'a mut [PointFlags],
    pub deltas: &'a mut [Point<Fixed>],
    pub iup_buffer: &'a mut [Point<Fixed>],
    pub composite_deltas: &'a mut [Point<Fixed>],
    #[cfg(feature = "hinting")]
    pub stack: &'a mut [i32],
    #[cfg(feature = "hinting")]
    pub cvt: &'a mut [i32],
    #[cfg(feature = "hinting")]
    pub storage: &'a mut [i32],
    #[cfg(feature = "hinting")]
    pub twilight_scaled: &'a mut [Point<F26Dot6>],
    #[cfg(feature = "hinting")]
    pub twilight_original_scaled: &'a mut [Point<F26Dot6>],
    #[cfg(feature = "hinting")]
    pub twilight_flags: &'a mut [PointFlags],
}

impl<'a> FreeTypeOutlineMemory<'a> {
    pub(super) fn new(outline: &Outline, buf: &'a mut [u8], hinting: Hinting) -> Option<Self> {
        #[cfg(feature = "hinting")]
        let hinted = outline.has_hinting && hinting == Hinting::Embedded;
        #[cfg(not(feature = "hinting"))]
        let _ = hinting;
        let (scaled, buf) = take_slice(buf, outline.points)?;
        let (unscaled, buf) = take_slice(buf, outline.max_other_points)?;
        // We only need original scaled points when hinting
        #[cfg(feature = "hinting")]
        let (original_scaled, buf) = if hinted {
            take_slice(buf, outline.max_other_points)?
        } else {
            (Default::default(), buf)
        };
        // Don't allocate any delta buffers if we don't have variations
        let (deltas, iup_buffer, composite_deltas, buf) = if outline.has_variations {
            let (deltas, buf) = take_slice(buf, outline.max_simple_points)?;
            let (iup_buffer, buf) = take_slice(buf, outline.max_simple_points)?;
            let (composite_deltas, buf) = take_slice(buf, outline.max_component_delta_stack)?;
            (deltas, iup_buffer, composite_deltas, buf)
        } else {
            (
                Default::default(),
                Default::default(),
                Default::default(),
                buf,
            )
        };
        // Hinting value stack
        #[cfg(feature = "hinting")]
        let (stack, buf) = if hinted {
            take_slice(buf, outline.max_stack)?
        } else {
            (Default::default(), buf)
        };
        // Copy-on-write buffers for CVT and storage area
        #[cfg(feature = "hinting")]
        let (cvt, storage, buf) = if hinted {
            let (cvt, buf) = take_slice(buf, outline.cvt_count)?;
            let (storage, buf) = take_slice(buf, outline.storage_count)?;
            (cvt, storage, buf)
        } else {
            (Default::default(), Default::default(), buf)
        };
        // Twilight zone point buffers
        #[cfg(feature = "hinting")]
        let (twilight_scaled, twilight_original_scaled, buf) = if hinted {
            let (scaled, buf) = take_slice(buf, outline.max_twilight_points)?;
            let (original_scaled, buf) = take_slice(buf, outline.max_twilight_points)?;
            (scaled, original_scaled, buf)
        } else {
            (Default::default(), Default::default(), buf)
        };
        let (contours, buf) = take_slice(buf, outline.contours)?;
        #[cfg(feature = "hinting")]
        let (flags, buf) = take_slice(buf, outline.points)?;
        #[cfg(not(feature = "hinting"))]
        let (flags, _) = take_slice(buf, outline.points)?;
        // Twilight zone point flags
        #[cfg(feature = "hinting")]
        let twilight_flags = if hinted {
            take_slice(buf, outline.max_twilight_points)?.0
        } else {
            Default::default()
        };
        Some(Self {
            unscaled,
            scaled,
            #[cfg(feature = "hinting")]
            original_scaled,
            contours,
            flags,
            deltas,
            iup_buffer,
            composite_deltas,
            #[cfg(feature = "hinting")]
            stack,
            #[cfg(feature = "hinting")]
            cvt,
            #[cfg(feature = "hinting")]
            storage,
            #[cfg(feature = "hinting")]
            twilight_scaled,
            #[cfg(feature = "hinting")]
            twilight_original_scaled,
            #[cfg(feature = "hinting")]
            twilight_flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_memory() {
        let outline_info = Outline {
            glyph: None,
            glyph_id: Default::default(),
            points: 10,
            contours: 4,
            max_simple_points: 4,
            max_other_points: 4,
            max_component_delta_stack: 4,
            max_stack: 0,
            cvt_count: 0,
            storage_count: 0,
            max_twilight_points: 0,
            has_hinting: false,
            has_variations: true,
            has_overlaps: false,
        };
        let required_size = outline_info.required_buffer_size(Hinting::None);
        let mut buf = vec![0u8; required_size];
        let memory = FreeTypeOutlineMemory::new(&outline_info, &mut buf, Hinting::None).unwrap();
        assert_eq!(memory.scaled.len(), outline_info.points);
        assert_eq!(memory.unscaled.len(), outline_info.max_other_points);
        // We don't allocate this buffer when hinting is disabled
        assert_eq!(memory.original_scaled.len(), 0);
        assert_eq!(memory.flags.len(), outline_info.points);
        assert_eq!(memory.contours.len(), outline_info.contours);
        assert_eq!(memory.deltas.len(), outline_info.max_simple_points);
        assert_eq!(memory.iup_buffer.len(), outline_info.max_simple_points);
        assert_eq!(
            memory.composite_deltas.len(),
            outline_info.max_component_delta_stack
        );
    }

    #[test]
    fn fail_outline_memory() {
        let outline_info = Outline {
            glyph: None,
            glyph_id: Default::default(),
            points: 10,
            contours: 4,
            max_simple_points: 4,
            max_other_points: 4,
            max_component_delta_stack: 4,
            max_stack: 0,
            cvt_count: 0,
            storage_count: 0,
            max_twilight_points: 0,
            has_hinting: false,
            has_variations: true,
            has_overlaps: false,
        };
        // Required size adds 4 bytes slop to account for internal alignment
        // requirements. So subtract 5 to force a failure.
        let not_enough = outline_info.required_buffer_size(Hinting::None) - 5;
        let mut buf = vec![0u8; not_enough];
        assert!(FreeTypeOutlineMemory::new(&outline_info, &mut buf, Hinting::None).is_none());
    }
}
