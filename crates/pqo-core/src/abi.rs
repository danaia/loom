use serde::{Deserialize, Serialize};

use crate::{DataType, ScalarType};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbiLayoutClass {
    PackedStream,
    ViewConstant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiLayout {
    pub size: u32,
    pub alignment: u32,
    pub fields: Vec<AbiFieldLayout>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiFieldLayout {
    pub name: String,
    pub offset: u32,
    pub layout: Box<AbiLayout>,
}

pub fn layout_of(data_type: &DataType, class: AbiLayoutClass) -> Result<AbiLayout, String> {
    match data_type {
        DataType::Scalar(scalar) => scalar_layout(scalar),
        DataType::Vector { scalar, lanes } => {
            if !(2..=4).contains(lanes) {
                return Err(format!("vector lane count {lanes} is outside 2..=4"));
            }
            let scalar = scalar_layout(scalar)?;
            let size = scalar.size * u32::from(*lanes);
            let alignment = match class {
                AbiLayoutClass::PackedStream => scalar.alignment,
                AbiLayoutClass::ViewConstant => {
                    if *lanes == 2 {
                        scalar.alignment * 2
                    } else {
                        16
                    }
                }
            };
            Ok(AbiLayout {
                size,
                alignment,
                fields: Vec::new(),
            })
        }
        DataType::Matrix {
            scalar,
            rows,
            columns,
        } => {
            let vector = DataType::Vector {
                scalar: scalar.clone(),
                lanes: *rows,
            };
            let column = layout_of(&vector, AbiLayoutClass::ViewConstant)?;
            let stride = align_up(column.size, column.alignment.max(16));
            Ok(AbiLayout {
                size: stride * u32::from(*columns),
                alignment: column.alignment.max(16),
                fields: Vec::new(),
            })
        }
        DataType::Struct { fields, .. } => {
            let mut offset = 0;
            let mut alignment = 1;
            let mut layouts = Vec::with_capacity(fields.len());
            for field in fields {
                let layout = layout_of(&field.data_type, class)?;
                offset = align_up(offset, layout.alignment);
                layouts.push(AbiFieldLayout {
                    name: field.name.clone(),
                    offset,
                    layout: Box::new(layout.clone()),
                });
                offset += layout.size;
                alignment = alignment.max(layout.alignment);
            }
            if class == AbiLayoutClass::ViewConstant {
                alignment = alignment.max(16);
            }
            Ok(AbiLayout {
                size: align_up(offset, alignment),
                alignment,
                fields: layouts,
            })
        }
        DataType::TextureHandle | DataType::ViewHandle => {
            Err("opaque handles have no cross-language buffer ABI".to_owned())
        }
    }
}

fn scalar_layout(scalar: &ScalarType) -> Result<AbiLayout, String> {
    let size = match scalar {
        ScalarType::Bool => 1,
        ScalarType::F16 => 2,
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => 4,
    };
    Ok(AbiLayout {
        size,
        alignment: size,
        fields: Vec::new(),
    })
}

const fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_vec3_preserves_twelve_byte_stream_stride() {
        let layout = layout_of(&DataType::vec3_f32(), AbiLayoutClass::PackedStream).unwrap();
        assert_eq!((layout.size, layout.alignment), (12, 4));
    }

    #[test]
    fn view_vec3_requires_sixteen_byte_alignment() {
        let layout = layout_of(&DataType::vec3_f32(), AbiLayoutClass::ViewConstant).unwrap();
        assert_eq!((layout.size, layout.alignment), (12, 16));
    }
}
