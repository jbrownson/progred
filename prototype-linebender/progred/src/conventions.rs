//! Editor conventions: well-known node ids the editor treats
//! specially. The data layer knows nothing of these — they are to gid
//! what syntax highlighting is to ASCII. Minted once via uuidgen
//! (CSPRNG) on 2026-07-05.

use progred_graph::NodeId;

pub const NAME: NodeId = NodeId::from_u128(0xf8ac_c21e_3635_4e5a_9702_1ee4_8d29_fed8);
pub const HEAD: NodeId = NodeId::from_u128(0x2911_020a_5ae2_4608_8f82_b9a8_ee0b_fa5a);
pub const TAIL: NodeId = NodeId::from_u128(0x5da3_2792_f067_4030_9c62_e302_2c9d_756c);
pub const EMPTY: NodeId = NodeId::from_u128(0xb8fa_b4b9_2fb6_42ec_8f93_9e27_360d_b3c8);
