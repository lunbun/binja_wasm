use binaryninja::binary_view::{BinaryView, BinaryViewBase};
use binaryninja::custom_binary_view::{BinaryViewType, BinaryViewTypeBase, CustomBinaryViewType, CustomView, CustomViewBuilder};
use crate::binja::view::WebAssemblyView;

pub struct WebAssemblyViewType {
    handle: BinaryViewType,
}

impl WebAssemblyViewType {
    pub fn new(handle: BinaryViewType) -> Self {
        Self { handle }
    }
}

impl BinaryViewTypeBase for WebAssemblyViewType {
    fn is_valid_for(&self, data: &BinaryView) -> bool {
        let mut buf = [0; 8];
        let len = BinaryViewBase::read(data, &mut buf, 0);
        if len != 8 {
            return false;
        }

        buf == "\0asm\x01\0\0\0".as_bytes()
    }
}

impl AsRef<BinaryViewType> for WebAssemblyViewType {
    fn as_ref(&self) -> &BinaryViewType {
        &self.handle
    }
}

impl CustomBinaryViewType for WebAssemblyViewType {
    fn create_custom_view<'builder>(
        &self,
        data: &BinaryView,
        builder: CustomViewBuilder<'builder, Self>,
    ) -> binaryninja::binary_view::Result<CustomView<'builder>> {
        builder.create::<WebAssemblyView>(data, ())
    }
}
