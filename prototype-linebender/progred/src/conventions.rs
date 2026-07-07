//! Editor conventions: well-known node ids the editor treats
//! specially. The data layer knows nothing of these — they are to gid
//! what syntax highlighting is to ASCII. Minted once via uuidgen
//! (CSPRNG) on 2026-07-05. The cons-list ids (head/tail/empty) were
//! retired 2026-07-06 for ordered-identity position labels.

use progred_graph::NodeId;

pub const NAME: NodeId = NodeId::from_u128(0xf8ac_c21e_3635_4e5a_9702_1ee4_8d29_fed8);
