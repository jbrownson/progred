//! The opaque CSS named colors, plus the separately specified transparent keyword.
//! Values follow CSS Color 4 §6.1: <https://www.w3.org/TR/css-color-4/#named-colors>.

use super::vocabulary;
use crate::name;
use gid::{CellId, Cells, Value};

struct Named {
    id: CellId,
    name: &'static str,
    rgb: u32,
}

pub(super) fn insert(cells: &mut Cells) {
    for color in NAMED {
        cells.set_value(
            color.id,
            name::record(
                color.name,
                [(
                    vocabulary::RGB,
                    Value::from(vec![
                        (color.rgb >> 16) as u8,
                        (color.rgb >> 8) as u8,
                        color.rgb as u8,
                    ]),
                )],
            ),
        );
    }
    cells.set_value(
        TRANSPARENT,
        name::record(
            "transparent",
            [(vocabulary::RGBA, Value::from(vec![0, 0, 0, 0]))],
        ),
    );
}

const TRANSPARENT: CellId = CellId::from_u128(0x4ce5a853f1372321370c162c749b79a2);

const NAMED: &[Named] = &[
    Named {
        id: CellId::from_u128(0x7cefe4187a414b743e5d15c76628440c),
        name: "aliceblue",
        rgb: 0xf0f8ff,
    },
    Named {
        id: CellId::from_u128(0xed7f0e98544416109955ad9adbe7e7d9),
        name: "antiquewhite",
        rgb: 0xfaebd7,
    },
    Named {
        id: CellId::from_u128(0x700ce3df8ed0223246be655fd2b897f8),
        name: "aqua",
        rgb: 0x00ffff,
    },
    Named {
        id: CellId::from_u128(0x6f6bf6d36245903ef1fa5271e8227832),
        name: "aquamarine",
        rgb: 0x7fffd4,
    },
    Named {
        id: CellId::from_u128(0xa314685b99ff21956dc5019f2e07cc78),
        name: "azure",
        rgb: 0xf0ffff,
    },
    Named {
        id: CellId::from_u128(0xa378b586002b10a7b0fbd405ae25af36),
        name: "beige",
        rgb: 0xf5f5dc,
    },
    Named {
        id: CellId::from_u128(0xb3194dcb0e5838c5bbe2629c9f3d94bd),
        name: "bisque",
        rgb: 0xffe4c4,
    },
    Named {
        id: CellId::from_u128(0x61199f6625c08e4c48dd2c06edd73443),
        name: "black",
        rgb: 0x000000,
    },
    Named {
        id: CellId::from_u128(0xc446b68f8f8d5b1893397ce7e45d6fd9),
        name: "blanchedalmond",
        rgb: 0xffebcd,
    },
    Named {
        id: CellId::from_u128(0x8349ea7b0d96120e4d4a78b9191cc9e8),
        name: "blue",
        rgb: 0x0000ff,
    },
    Named {
        id: CellId::from_u128(0xe71011a71bc137399da7835fcc60c9e9),
        name: "blueviolet",
        rgb: 0x8a2be2,
    },
    Named {
        id: CellId::from_u128(0xd6005e83d0d60a36789fe6942bcfaea2),
        name: "brown",
        rgb: 0xa52a2a,
    },
    Named {
        id: CellId::from_u128(0x7b748067775a5db73f7f111193aef79b),
        name: "burlywood",
        rgb: 0xdeb887,
    },
    Named {
        id: CellId::from_u128(0x127fb12ca8d5951190e22d7e25f4b4a7),
        name: "cadetblue",
        rgb: 0x5f9ea0,
    },
    Named {
        id: CellId::from_u128(0xa25c93b5466dadd7c4eb01876f0d4ef9),
        name: "chartreuse",
        rgb: 0x7fff00,
    },
    Named {
        id: CellId::from_u128(0xc30449616e9da0b3ee6d92bc8def3a4c),
        name: "chocolate",
        rgb: 0xd2691e,
    },
    Named {
        id: CellId::from_u128(0xef9f96ce329c8ffcdbfa33f9ed0269c3),
        name: "coral",
        rgb: 0xff7f50,
    },
    Named {
        id: CellId::from_u128(0xc473004bb26235d48e8e837e33180991),
        name: "cornflowerblue",
        rgb: 0x6495ed,
    },
    Named {
        id: CellId::from_u128(0x0f9b947d64121c8cda1c8379b148c9b6),
        name: "cornsilk",
        rgb: 0xfff8dc,
    },
    Named {
        id: CellId::from_u128(0x4145cf06456e91aa0b2ac6b30de49b90),
        name: "crimson",
        rgb: 0xdc143c,
    },
    Named {
        id: CellId::from_u128(0xd1161902dfe798f73a1a33f50e40e6d2),
        name: "cyan",
        rgb: 0x00ffff,
    },
    Named {
        id: CellId::from_u128(0x49aab8bb5f50e9ff660b205a7192c6ce),
        name: "darkblue",
        rgb: 0x00008b,
    },
    Named {
        id: CellId::from_u128(0x436191651253dda8fa3e40b685f15733),
        name: "darkcyan",
        rgb: 0x008b8b,
    },
    Named {
        id: CellId::from_u128(0xc1a5821f3f0b352872777427496c0eaf),
        name: "darkgoldenrod",
        rgb: 0xb8860b,
    },
    Named {
        id: CellId::from_u128(0x88f0096da9aa1076333f1d6b46ad8b91),
        name: "darkgray",
        rgb: 0xa9a9a9,
    },
    Named {
        id: CellId::from_u128(0xcaac32be97763771870cf9ad1c45cfa0),
        name: "darkgreen",
        rgb: 0x006400,
    },
    Named {
        id: CellId::from_u128(0x61b93cce7a382ffc862b82f2c9824f2f),
        name: "darkgrey",
        rgb: 0xa9a9a9,
    },
    Named {
        id: CellId::from_u128(0x092ce0438a64f52a8985902922080c38),
        name: "darkkhaki",
        rgb: 0xbdb76b,
    },
    Named {
        id: CellId::from_u128(0x10cb0bf91badad46df0bcf09283c5876),
        name: "darkmagenta",
        rgb: 0x8b008b,
    },
    Named {
        id: CellId::from_u128(0x381b072cde28ad46ddd9e69cf1604d45),
        name: "darkolivegreen",
        rgb: 0x556b2f,
    },
    Named {
        id: CellId::from_u128(0x3779fcb50ff2cdba46b8c54f80d2192a),
        name: "darkorange",
        rgb: 0xff8c00,
    },
    Named {
        id: CellId::from_u128(0xfe4e4c1a806fe7653cdfa3659ede5e73),
        name: "darkorchid",
        rgb: 0x9932cc,
    },
    Named {
        id: CellId::from_u128(0xf41807c3f710382a9ca062a086ca6717),
        name: "darkred",
        rgb: 0x8b0000,
    },
    Named {
        id: CellId::from_u128(0xc5a2a33e7e2e1a83412f5b4cbf516574),
        name: "darksalmon",
        rgb: 0xe9967a,
    },
    Named {
        id: CellId::from_u128(0x0266ff389eae9aa85a62fe1840c82c6e),
        name: "darkseagreen",
        rgb: 0x8fbc8f,
    },
    Named {
        id: CellId::from_u128(0x7011011c45a2150de647a4c501628284),
        name: "darkslateblue",
        rgb: 0x483d8b,
    },
    Named {
        id: CellId::from_u128(0xbc7a5f397c1f72ecc477ad9f030dbd56),
        name: "darkslategray",
        rgb: 0x2f4f4f,
    },
    Named {
        id: CellId::from_u128(0x3434f9192d6728eca5e82ec8642e475d),
        name: "darkslategrey",
        rgb: 0x2f4f4f,
    },
    Named {
        id: CellId::from_u128(0xad06de28cc54db0810319a192e311a6c),
        name: "darkturquoise",
        rgb: 0x00ced1,
    },
    Named {
        id: CellId::from_u128(0x08efd51ed429d8cecf4f382fd375b309),
        name: "darkviolet",
        rgb: 0x9400d3,
    },
    Named {
        id: CellId::from_u128(0xaefa58aca2a8f0bcc5910de30a366edc),
        name: "deeppink",
        rgb: 0xff1493,
    },
    Named {
        id: CellId::from_u128(0x23cd8d368f86da7f8a0c3d6316f7cda3),
        name: "deepskyblue",
        rgb: 0x00bfff,
    },
    Named {
        id: CellId::from_u128(0x4c5c5749d7a8cf4b09608eeeb6ca8d0f),
        name: "dimgray",
        rgb: 0x696969,
    },
    Named {
        id: CellId::from_u128(0x12886b5383345e1b56afe4afa232bea0),
        name: "dimgrey",
        rgb: 0x696969,
    },
    Named {
        id: CellId::from_u128(0x43bef79754eedf188b34c85c14d399d3),
        name: "dodgerblue",
        rgb: 0x1e90ff,
    },
    Named {
        id: CellId::from_u128(0x20667ab7aa89f5329e3195ccb1da2362),
        name: "firebrick",
        rgb: 0xb22222,
    },
    Named {
        id: CellId::from_u128(0x4fbcc7b0309acf3ff5f5f7e67f0e7f6f),
        name: "floralwhite",
        rgb: 0xfffaf0,
    },
    Named {
        id: CellId::from_u128(0x4a13b1de9fb20df907fd0d890958719d),
        name: "forestgreen",
        rgb: 0x228b22,
    },
    Named {
        id: CellId::from_u128(0x41f7af5c6d68174bab4d0b7f7f01280f),
        name: "fuchsia",
        rgb: 0xff00ff,
    },
    Named {
        id: CellId::from_u128(0x54a21ef6dccc3d92c9f2b1fe8e1780a8),
        name: "gainsboro",
        rgb: 0xdcdcdc,
    },
    Named {
        id: CellId::from_u128(0xb4860e0b8d2518a0301eb99e1a033ebd),
        name: "ghostwhite",
        rgb: 0xf8f8ff,
    },
    Named {
        id: CellId::from_u128(0x496c5c6507c5360592648a06d56084d2),
        name: "gold",
        rgb: 0xffd700,
    },
    Named {
        id: CellId::from_u128(0x99e2f728a0cf8e44e6ca126c5645ad3e),
        name: "goldenrod",
        rgb: 0xdaa520,
    },
    Named {
        id: CellId::from_u128(0xaea4b24a58ce18abfd5963a45b4de0ca),
        name: "gray",
        rgb: 0x808080,
    },
    Named {
        id: CellId::from_u128(0x9d52affaa4ab47c661cf893e3174d693),
        name: "green",
        rgb: 0x008000,
    },
    Named {
        id: CellId::from_u128(0x8289681da512b8a7f10f1733283e7fdf),
        name: "greenyellow",
        rgb: 0xadff2f,
    },
    Named {
        id: CellId::from_u128(0xd2fb236bb621777c22d8ed4c5b4a103b),
        name: "grey",
        rgb: 0x808080,
    },
    Named {
        id: CellId::from_u128(0x69556f8da0cd2f4b77f1265f0e915f84),
        name: "honeydew",
        rgb: 0xf0fff0,
    },
    Named {
        id: CellId::from_u128(0x7432348164cb887b496f42f701f5ebdf),
        name: "hotpink",
        rgb: 0xff69b4,
    },
    Named {
        id: CellId::from_u128(0x351e1bf01e4ef6a26827a709b3a1bba8),
        name: "indianred",
        rgb: 0xcd5c5c,
    },
    Named {
        id: CellId::from_u128(0x3233f5b4b66d8282c663f273d102b1b9),
        name: "indigo",
        rgb: 0x4b0082,
    },
    Named {
        id: CellId::from_u128(0xe59b682da026fdd20808707552024131),
        name: "ivory",
        rgb: 0xfffff0,
    },
    Named {
        id: CellId::from_u128(0x6f5bc1cf6d20c5f03ed0d4b8898b2d6f),
        name: "khaki",
        rgb: 0xf0e68c,
    },
    Named {
        id: CellId::from_u128(0xd1d3acae7d09fc4ee2f2b638fc19ebac),
        name: "lavender",
        rgb: 0xe6e6fa,
    },
    Named {
        id: CellId::from_u128(0x750851fd375cf638be1a2e13308be446),
        name: "lavenderblush",
        rgb: 0xfff0f5,
    },
    Named {
        id: CellId::from_u128(0xe36b9acd0149c185a3c7c7a62b938ebd),
        name: "lawngreen",
        rgb: 0x7cfc00,
    },
    Named {
        id: CellId::from_u128(0xc6e0fb6c94bfa623313701c1c72bad8d),
        name: "lemonchiffon",
        rgb: 0xfffacd,
    },
    Named {
        id: CellId::from_u128(0x6a7a1b66566b00f91a3a622520f05ca4),
        name: "lightblue",
        rgb: 0xadd8e6,
    },
    Named {
        id: CellId::from_u128(0x7c239e3685aa305a604b9e45f02fe883),
        name: "lightcoral",
        rgb: 0xf08080,
    },
    Named {
        id: CellId::from_u128(0x7d17a3187241dc6142ae08afe6c4531b),
        name: "lightcyan",
        rgb: 0xe0ffff,
    },
    Named {
        id: CellId::from_u128(0x5b97449dce940f6ec4f237198bb163e0),
        name: "lightgoldenrodyellow",
        rgb: 0xfafad2,
    },
    Named {
        id: CellId::from_u128(0x5d2380a3a89d1ad9938c74551f0c98fb),
        name: "lightgray",
        rgb: 0xd3d3d3,
    },
    Named {
        id: CellId::from_u128(0xc2a2c11c44dea9db77834a5f73c2b122),
        name: "lightgreen",
        rgb: 0x90ee90,
    },
    Named {
        id: CellId::from_u128(0x06a7f2747e2de9ee7d930095575d1f30),
        name: "lightgrey",
        rgb: 0xd3d3d3,
    },
    Named {
        id: CellId::from_u128(0x5d49c1b0073f357969c0b708419ec20c),
        name: "lightpink",
        rgb: 0xffb6c1,
    },
    Named {
        id: CellId::from_u128(0x23acf42eeb9dcbbba6c1522d866d1a82),
        name: "lightsalmon",
        rgb: 0xffa07a,
    },
    Named {
        id: CellId::from_u128(0x8d187ed9b8fc3a80e457f5b3e565a5fa),
        name: "lightseagreen",
        rgb: 0x20b2aa,
    },
    Named {
        id: CellId::from_u128(0xe36b7e1e4521a696a8541d081ebde091),
        name: "lightskyblue",
        rgb: 0x87cefa,
    },
    Named {
        id: CellId::from_u128(0xfe1d8b45cf15829c0a4c65e2ab024ae2),
        name: "lightslategray",
        rgb: 0x778899,
    },
    Named {
        id: CellId::from_u128(0x0a1b2919f53ca40f7e73b60d389ff33b),
        name: "lightslategrey",
        rgb: 0x778899,
    },
    Named {
        id: CellId::from_u128(0x49c6d375acfe3afdfdab694129df4263),
        name: "lightsteelblue",
        rgb: 0xb0c4de,
    },
    Named {
        id: CellId::from_u128(0xc30ec0792c2fbcd7ff11db450d1606ef),
        name: "lightyellow",
        rgb: 0xffffe0,
    },
    Named {
        id: CellId::from_u128(0xd2d41028d7dbd7c89ea535b4809e071b),
        name: "lime",
        rgb: 0x00ff00,
    },
    Named {
        id: CellId::from_u128(0xcc8d8e9f91be6964fd067dc947e50f39),
        name: "limegreen",
        rgb: 0x32cd32,
    },
    Named {
        id: CellId::from_u128(0x58e2acec1f6da2a339cd2472e52bf007),
        name: "linen",
        rgb: 0xfaf0e6,
    },
    Named {
        id: CellId::from_u128(0xfe4241b20ab7495781bbcf19eebcd807),
        name: "magenta",
        rgb: 0xff00ff,
    },
    Named {
        id: CellId::from_u128(0x99e0511244fa7f3397aee97b99833c29),
        name: "maroon",
        rgb: 0x800000,
    },
    Named {
        id: CellId::from_u128(0xcf960f2881918f9f6b3b2c7876067579),
        name: "mediumaquamarine",
        rgb: 0x66cdaa,
    },
    Named {
        id: CellId::from_u128(0x6331303f9f01771669bd7621b4ebe7ab),
        name: "mediumblue",
        rgb: 0x0000cd,
    },
    Named {
        id: CellId::from_u128(0xc3e9bf83a1b8fdcc6b24dd92fb734cc7),
        name: "mediumorchid",
        rgb: 0xba55d3,
    },
    Named {
        id: CellId::from_u128(0x57907c90f7256b7e46f0b5d9e2396936),
        name: "mediumpurple",
        rgb: 0x9370db,
    },
    Named {
        id: CellId::from_u128(0xc26e7e2fe313b1edf00b69fd4f2aac1e),
        name: "mediumseagreen",
        rgb: 0x3cb371,
    },
    Named {
        id: CellId::from_u128(0xf394707bf30118e69daf9f917010a8e9),
        name: "mediumslateblue",
        rgb: 0x7b68ee,
    },
    Named {
        id: CellId::from_u128(0xe7c7ea0cd75d7678a94c8f05671ff3ad),
        name: "mediumspringgreen",
        rgb: 0x00fa9a,
    },
    Named {
        id: CellId::from_u128(0xad25608b0b0330b6de8be9c092b9d186),
        name: "mediumturquoise",
        rgb: 0x48d1cc,
    },
    Named {
        id: CellId::from_u128(0xf3bff8263a5db14d6e9e9c4c3e409f3a),
        name: "mediumvioletred",
        rgb: 0xc71585,
    },
    Named {
        id: CellId::from_u128(0xa2fcbfdf3d14a67bfdd7e900f46f5b42),
        name: "midnightblue",
        rgb: 0x191970,
    },
    Named {
        id: CellId::from_u128(0xa163170944e1b0b0e6e287083605339b),
        name: "mintcream",
        rgb: 0xf5fffa,
    },
    Named {
        id: CellId::from_u128(0x29fdb7bbd4c656fd4352d92c8853fc29),
        name: "mistyrose",
        rgb: 0xffe4e1,
    },
    Named {
        id: CellId::from_u128(0x3a07af33eabf160f920b7b4ada02a16c),
        name: "moccasin",
        rgb: 0xffe4b5,
    },
    Named {
        id: CellId::from_u128(0xa16e38ef48ff758dc5bfd44b6483046e),
        name: "navajowhite",
        rgb: 0xffdead,
    },
    Named {
        id: CellId::from_u128(0xb440caa995426b1e786ad06c7204a342),
        name: "navy",
        rgb: 0x000080,
    },
    Named {
        id: CellId::from_u128(0x62b60973b7ee2522f10090e8ef4b91b9),
        name: "oldlace",
        rgb: 0xfdf5e6,
    },
    Named {
        id: CellId::from_u128(0xf2076e4def36f2a09ffbdc7d21a2c05d),
        name: "olive",
        rgb: 0x808000,
    },
    Named {
        id: CellId::from_u128(0xb5eaacfa1c57c586d8259ac50d315b70),
        name: "olivedrab",
        rgb: 0x6b8e23,
    },
    Named {
        id: CellId::from_u128(0x7efa88967082e656bc0e45d4ac64e018),
        name: "orange",
        rgb: 0xffa500,
    },
    Named {
        id: CellId::from_u128(0x32b9687e056a6661734c55df2c35b175),
        name: "orangered",
        rgb: 0xff4500,
    },
    Named {
        id: CellId::from_u128(0x8d97f59c17ca6f95cee13539ccb8a6ad),
        name: "orchid",
        rgb: 0xda70d6,
    },
    Named {
        id: CellId::from_u128(0xf6c11e8763376be4a0f95cfc9404e34d),
        name: "palegoldenrod",
        rgb: 0xeee8aa,
    },
    Named {
        id: CellId::from_u128(0x6e123e91f570264052d3d826bacf0e94),
        name: "palegreen",
        rgb: 0x98fb98,
    },
    Named {
        id: CellId::from_u128(0x12ed7c775ac238dad28b0b294b044c49),
        name: "paleturquoise",
        rgb: 0xafeeee,
    },
    Named {
        id: CellId::from_u128(0x81a73b11dbab535ea7290e1c96a3a8ba),
        name: "palevioletred",
        rgb: 0xdb7093,
    },
    Named {
        id: CellId::from_u128(0x975ea9ffcaf7dffc1ee6d81d03613de1),
        name: "papayawhip",
        rgb: 0xffefd5,
    },
    Named {
        id: CellId::from_u128(0x6e198450726f9ede116b42d7d3957732),
        name: "peachpuff",
        rgb: 0xffdab9,
    },
    Named {
        id: CellId::from_u128(0x2502897f533abe19160651984ed94355),
        name: "peru",
        rgb: 0xcd853f,
    },
    Named {
        id: CellId::from_u128(0xd526653a29ce8c68046570682f06907a),
        name: "pink",
        rgb: 0xffc0cb,
    },
    Named {
        id: CellId::from_u128(0xb1ae54e8a8910237d5037aace6971f8c),
        name: "plum",
        rgb: 0xdda0dd,
    },
    Named {
        id: CellId::from_u128(0x90d9dfc2b4ee6e2d4d2ed7b021760c6b),
        name: "powderblue",
        rgb: 0xb0e0e6,
    },
    Named {
        id: CellId::from_u128(0xa7fc8d0fe25ec802a92dce2b9c1c8f3b),
        name: "purple",
        rgb: 0x800080,
    },
    Named {
        id: CellId::from_u128(0xac0f8aa50d9f5db91c66acf533ca572f),
        name: "rebeccapurple",
        rgb: 0x663399,
    },
    Named {
        id: CellId::from_u128(0x98515abc4716c0266c627daa959616dc),
        name: "red",
        rgb: 0xff0000,
    },
    Named {
        id: CellId::from_u128(0xe4448c7702340baf2159ba527bc16c80),
        name: "rosybrown",
        rgb: 0xbc8f8f,
    },
    Named {
        id: CellId::from_u128(0xb7e782e6df3f4e0186948da98f2ac0f6),
        name: "royalblue",
        rgb: 0x4169e1,
    },
    Named {
        id: CellId::from_u128(0x55f987d332bce52b6fe08edc67c83cd7),
        name: "saddlebrown",
        rgb: 0x8b4513,
    },
    Named {
        id: CellId::from_u128(0xee2b3d8722d962e02b483adb5da2e224),
        name: "salmon",
        rgb: 0xfa8072,
    },
    Named {
        id: CellId::from_u128(0xbbaa2fbd3a67692526cc1750bedbf491),
        name: "sandybrown",
        rgb: 0xf4a460,
    },
    Named {
        id: CellId::from_u128(0xc517e86a20ff593a2c0adac739731f84),
        name: "seagreen",
        rgb: 0x2e8b57,
    },
    Named {
        id: CellId::from_u128(0xa3949401edeec5899cd222188154f6ba),
        name: "seashell",
        rgb: 0xfff5ee,
    },
    Named {
        id: CellId::from_u128(0x4a01464c7c7111f126514355d51a04ba),
        name: "sienna",
        rgb: 0xa0522d,
    },
    Named {
        id: CellId::from_u128(0x7cd24dbf2a3d6d9e43904db0ed814e2c),
        name: "silver",
        rgb: 0xc0c0c0,
    },
    Named {
        id: CellId::from_u128(0x3d4c3095b801120988f85260efd77591),
        name: "skyblue",
        rgb: 0x87ceeb,
    },
    Named {
        id: CellId::from_u128(0x00939e57b96a2f26f926bb2d04f1b291),
        name: "slateblue",
        rgb: 0x6a5acd,
    },
    Named {
        id: CellId::from_u128(0xdea03784dff932be59e3f60c52fdf136),
        name: "slategray",
        rgb: 0x708090,
    },
    Named {
        id: CellId::from_u128(0xabe9efb0b0f0e59e3f361648c52872a7),
        name: "slategrey",
        rgb: 0x708090,
    },
    Named {
        id: CellId::from_u128(0xb0b6a8ff416bec708f7b66258d11031e),
        name: "snow",
        rgb: 0xfffafa,
    },
    Named {
        id: CellId::from_u128(0xda37686dd6cb37f750c82705a384b05d),
        name: "springgreen",
        rgb: 0x00ff7f,
    },
    Named {
        id: CellId::from_u128(0x54681bc2c167977979f284fe5d415a0d),
        name: "steelblue",
        rgb: 0x4682b4,
    },
    Named {
        id: CellId::from_u128(0x0c6e418a1a1cd7dfb759972ca671f078),
        name: "tan",
        rgb: 0xd2b48c,
    },
    Named {
        id: CellId::from_u128(0x114a85b4db549d6b2e44e725cf6c452a),
        name: "teal",
        rgb: 0x008080,
    },
    Named {
        id: CellId::from_u128(0xc62ddcaa2b3f9bbce73326c5bf62d6d9),
        name: "thistle",
        rgb: 0xd8bfd8,
    },
    Named {
        id: CellId::from_u128(0x1354222b8edb03779d325f1db486c91a),
        name: "tomato",
        rgb: 0xff6347,
    },
    Named {
        id: CellId::from_u128(0x2585cabb34c7a908ef6db7df22c20bea),
        name: "turquoise",
        rgb: 0x40e0d0,
    },
    Named {
        id: CellId::from_u128(0x7277b08ced5cfe9060152e93ef1cd8c6),
        name: "violet",
        rgb: 0xee82ee,
    },
    Named {
        id: CellId::from_u128(0x03a2bbed1efe52adc724d210616c55cb),
        name: "wheat",
        rgb: 0xf5deb3,
    },
    Named {
        id: CellId::from_u128(0x717b67edde9de004bfe5de5edf7daa5e),
        name: "white",
        rgb: 0xffffff,
    },
    Named {
        id: CellId::from_u128(0x1e0396e2a0b8a75b36713ce5fe490c39),
        name: "whitesmoke",
        rgb: 0xf5f5f5,
    },
    Named {
        id: CellId::from_u128(0x5a7d44398d29e7b5ef24cec9d0b90874),
        name: "yellow",
        rgb: 0xffff00,
    },
    Named {
        id: CellId::from_u128(0x6674bd3cc3a834f10d693871031b6a7a),
        name: "yellowgreen",
        rgb: 0x9acd32,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{color, name};
    use std::collections::BTreeSet;

    #[test]
    fn the_css_palette_has_unique_durable_cells_and_expected_aliases() {
        assert_eq!(NAMED.len(), 148);
        assert_eq!(
            NAMED
                .iter()
                .map(|color| color.id)
                .chain([TRANSPARENT])
                .collect::<BTreeSet<_>>()
                .len(),
            149,
        );

        let mut cells = Cells::new();
        insert(&mut cells);
        let named = |wanted| {
            cells
                .iter()
                .find_map(|(cell, value)| (name::read(value) == Some(wanted)).then_some(*cell))
                .and_then(|cell| cells.value(cell))
                .and_then(color::read)
        };
        assert_eq!(
            named("rebeccapurple"),
            Some(puri::Color::from_rgb8(0x66, 0x33, 0x99))
        );
        assert_eq!(named("aqua"), named("cyan"));
        assert_eq!(named("transparent"), Some(puri::Color::TRANSPARENT));
    }
}
