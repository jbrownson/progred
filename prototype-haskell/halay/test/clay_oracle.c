#define CLAY_IMPLEMENTATION

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vendor/clay/clay.h"

static void handle_clay_error(Clay_ErrorData error_data) {
    fprintf(stderr, "%.*s\n", error_data.errorText.length, error_data.errorText.chars);
}

static Clay_Dimensions measure_text(Clay_StringSlice text, Clay_TextElementConfig *config, void *user_data) {
    (void)user_data;
    return (Clay_Dimensions) {
        .width = (float)text.length,
        .height = config->fontSize > 0 ? (float)config->fontSize : 1.0f,
    };
}

static void init_clay(void) {
    uint64_t total_memory_size = Clay_MinMemorySize();
    Clay_Arena arena = Clay_CreateArenaWithCapacityAndMemory(total_memory_size, malloc(total_memory_size));
    Clay_Initialize(
        arena,
        (Clay_Dimensions) {.width = 1000, .height = 1000},
        (Clay_ErrorHandler) {.errorHandlerFunction = handle_clay_error});
    Clay_SetMeasureTextFunction(measure_text, NULL);
}

static Clay_Sizing fixed_size(float width, float height) {
    return (Clay_Sizing) {
        .width = CLAY_SIZING_FIXED(width),
        .height = CLAY_SIZING_FIXED(height),
    };
}

static Clay_SizingAxis sizing_axis_from_int(int type_value, float value, float min_value, float max_value) {
    Clay_SizingAxis axis = {0};
    switch (type_value) {
        case 0:
            axis.type = CLAY__SIZING_TYPE_FIT;
            axis.size.minMax.min = min_value;
            axis.size.minMax.max = max_value;
            return axis;
        case 1:
            axis.type = CLAY__SIZING_TYPE_FIXED;
            axis.size.minMax.min = value;
            axis.size.minMax.max = value;
            return axis;
        case 2:
            axis.type = CLAY__SIZING_TYPE_GROW;
            axis.size.minMax.min = min_value;
            axis.size.minMax.max = max_value;
            return axis;
        default:
            axis.type = CLAY__SIZING_TYPE_PERCENT;
            axis.size.percent = value;
            return axis;
    }
}

#define RANDOM_CHILD(index, id_name)                                                                                                 \
    CLAY(CLAY_ID(id_name), {                                                                                                         \
        .layout = {                                                                                                                  \
            .sizing = {                                                                                                              \
                .width = sizing_axis_from_int(child_width_sizing_types[index], child_width_sizing_values[index], child_width_sizing_mins[index], child_width_sizing_maxes[index]),       \
                .height = sizing_axis_from_int(child_height_sizing_types[index], child_height_sizing_values[index], child_height_sizing_mins[index], child_height_sizing_maxes[index]), \
            },                                                                                                                       \
        },                                                                                                                           \
        .aspectRatio = {.aspectRatio = child_aspect_ratios[index]},                                                                  \
    }) {                                                                                                                             \
        CLAY_AUTO_ID({.layout = {.sizing = fixed_size(child_widths[index], child_heights[index])}}) {}                                \
    }

static void emit_rect(const char *case_name, const char *id_name) {
    Clay_ElementData data = Clay_GetElementData(Clay_GetElementId((Clay_String) {
        .length = (int32_t)strlen(id_name),
        .chars = id_name,
    }));
    if (!data.found) {
        fprintf(stderr, "missing element %s in case %s\n", id_name, case_name);
        exit(1);
    }
    printf(
        "%s %s %.4f %.4f %.4f %.4f\n",
        case_name,
        id_name,
        data.boundingBox.x,
        data.boundingBox.y,
        data.boundingBox.width,
        data.boundingBox.height);
}

static void emit_case(const char *case_name, const char **ids, int id_count) {
    Clay_EndLayout(0);
    for (int i = 0; i < id_count; i++) {
        emit_rect(case_name, ids[i]);
    }
}

static void row_gap_and_padding(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .padding = {.left = 5, .right = 3, .top = 7, .bottom = 5},
            .childGap = 3,
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(10, 5)}}) {}
        CLAY(CLAY_ID("b"), {.layout = {.sizing = fixed_size(20, 8)}}) {}
    }
    emit_case("row_gap_and_padding", ids, 3);
}

static void column_gap_and_padding(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .layoutDirection = CLAY_TOP_TO_BOTTOM,
            .padding = {.left = 2, .right = 8, .top = 3, .bottom = 10},
            .childGap = 4,
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(10, 5)}}) {}
        CLAY(CLAY_ID("b"), {.layout = {.sizing = fixed_size(20, 8)}}) {}
    }
    emit_case("column_gap_and_padding", ids, 3);
}

static void fixed_box_centers_child(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(100, 50),
            .childAlignment = {.x = CLAY_ALIGN_X_CENTER, .y = CLAY_ALIGN_Y_CENTER},
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(20, 10)}}) {}
    }
    emit_case("fixed_box_centers_child", ids, 2);
}

static void percent_child(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(200, 20)}}) {
        CLAY(CLAY_ID("a"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_PERCENT(0.5f),
                    .height = CLAY_SIZING_FIXED(10),
                },
            },
        }) {}
        CLAY(CLAY_ID("b"), {.layout = {.sizing = fixed_size(20, 10)}}) {}
    }
    emit_case("percent_child", ids, 3);
}

static void grow_main_axis(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(100, 20),
            .childGap = 10,
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(20, 10)}}) {}
        CLAY(CLAY_ID("b"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_GROW(0),
                    .height = CLAY_SIZING_FIXED(10),
                },
            },
        }) {}
    }
    emit_case("grow_main_axis", ids, 3);
}

static void grow_cross_axis(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(100, 50)}}) {
        CLAY(CLAY_ID("a"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_FIXED(10),
                    .height = CLAY_SIZING_GROW(0),
                },
            },
        }) {}
    }
    emit_case("grow_cross_axis", ids, 2);
}

static void clamp_grow(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(100, 20)}}) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(20, 10)}}) {}
        CLAY(CLAY_ID("b"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_GROW(0, 30),
                    .height = CLAY_SIZING_FIXED(10),
                },
            },
        }) {}
    }
    emit_case("clamp_grow", ids, 3);
}

static void aspect_ratio_width_drives_height(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(100, 100)}}) {
        CLAY(CLAY_ID("a"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_FIXED(40),
                },
            },
            .aspectRatio = {.aspectRatio = 2.0f},
        }) {}
    }
    emit_case("aspect_ratio_width_drives_height", ids, 2);
}

static void aspect_ratio_height_drives_width(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(100, 100),
            .layoutDirection = CLAY_TOP_TO_BOTTOM,
        },
    }) {
        CLAY(CLAY_ID("a"), {
            .layout = {
                .sizing = {
                    .height = CLAY_SIZING_FIXED(30),
                },
            },
            .aspectRatio = {.aspectRatio = 2.0f},
        }) {}
    }
    emit_case("aspect_ratio_height_drives_width", ids, 2);
}

static void unequal_grow_main_axis(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(1, 4),
            .layoutDirection = CLAY_TOP_TO_BOTTOM,
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = {.width = CLAY_SIZING_FIT(), .height = CLAY_SIZING_GROW(0)}}}) {
            CLAY_AUTO_ID({.layout = {.sizing = fixed_size(1, 1)}}) {}
        }
        CLAY(CLAY_ID("b"), {.layout = {.sizing = {.width = CLAY_SIZING_FIT(), .height = CLAY_SIZING_GROW(0)}}}) {
            CLAY_AUTO_ID({.layout = {.sizing = fixed_size(1, 2)}}) {}
        }
    }
    emit_case("unequal_grow_main_axis", ids, 3);
}

static void nested_box_positions_children(void) {
    const char *ids[] = {"root", "a", "b", "c"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(120, 80),
            .padding = {.left = 4, .right = 7, .top = 3, .bottom = 5},
            .childGap = 6,
        },
    }) {
        CLAY_AUTO_ID({
            .layout = {
                .layoutDirection = CLAY_TOP_TO_BOTTOM,
                .padding = {.left = 3, .right = 2, .top = 5, .bottom = 4},
                .childGap = 2,
            },
        }) {
            CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(10, 5)}}) {}
            CLAY(CLAY_ID("b"), {.layout = {.sizing = fixed_size(20, 8)}}) {}
        }
        CLAY(CLAY_ID("c"), {.layout = {.sizing = fixed_size(15, 7)}}) {}
    }
    emit_case("nested_box_positions_children", ids, 4);
}

static void overflow_cross_center(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(10, 10),
            .childAlignment = {.y = CLAY_ALIGN_Y_CENTER},
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(5, 20)}}) {}
    }
    emit_case("overflow_cross_center", ids, 2);
}

static void clip_main_axis_does_not_compress(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {.sizing = fixed_size(6, 20)},
        .clip = {.horizontal = true},
    }) {
        CLAY(CLAY_ID("a"), {}) {
            CLAY_TEXT(CLAY_STRING("aaaaa bbbbb"), CLAY_TEXT_CONFIG({.fontSize = 1}));
        }
    }
    emit_case("clip_main_axis_does_not_compress", ids, 2);
}

static void clip_cross_axis_grows_to_content(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {.sizing = fixed_size(100, 10)},
        .clip = {.vertical = true},
    }) {
        CLAY(CLAY_ID("a"), {
            .layout = {
                .sizing = {
                    .height = CLAY_SIZING_GROW(0),
                },
            },
            .clip = {.vertical = true},
        }) {
            CLAY_AUTO_ID({.layout = {.sizing = fixed_size(5, 20)}}) {}
        }
    }
    emit_case("clip_cross_axis_grows_to_content", ids, 2);
}

static void clip_cross_axis_uses_pre_percent_inner_size(void) {
    const char *ids[] = {"root", "a", "b"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(73, 80),
            .layoutDirection = CLAY_TOP_TO_BOTTOM,
            .padding = {.left = 12, .right = 7},
            .childAlignment = {.x = CLAY_ALIGN_X_CENTER},
        },
        .clip = {
            .horizontal = true,
            .vertical = true,
        },
    }) {
        CLAY(CLAY_ID("a"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_GROW(0),
                    .height = CLAY_SIZING_FIXED(67),
                },
                .layoutDirection = CLAY_TOP_TO_BOTTOM,
                .padding = {.top = 5, .bottom = 18},
            },
            .clip = {.vertical = true},
        }) {
            CLAY_TEXT(
                CLAY_STRING("xx xxxxxxxxxxxxxxxxxxx xxxxxxxxxxxxxx xxx"),
                CLAY_TEXT_CONFIG({
                    .fontSize = 1,
                    .wrapMode = CLAY_TEXT_WRAP_NONE,
                    .textAlignment = CLAY_TEXT_ALIGN_CENTER,
                }));
        }
        CLAY(CLAY_ID("b"), {
            .layout = {
                .sizing = {
                    .width = CLAY_SIZING_PERCENT(0.84f),
                    .height = CLAY_SIZING_FIXED(31),
                },
            },
            .aspectRatio = {.aspectRatio = 1.8f},
        }) {
            CLAY_AUTO_ID({.layout = {.sizing = fixed_size(3, 26)}}) {}
        }
    }
    emit_case("clip_cross_axis_uses_pre_percent_inner_size", ids, 3);
}

static void clip_child_offset_places_children(void) {
    const char *ids[] = {"root", "a"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(50, 50),
            .padding = {.left = 5, .top = 6},
        },
        .clip = {
            .horizontal = true,
            .vertical = true,
            .childOffset = {-3, 7},
        },
    }) {
        CLAY(CLAY_ID("a"), {.layout = {.sizing = fixed_size(10, 10)}}) {}
    }
    emit_case("clip_child_offset_places_children", ids, 2);
}

static void emit_text_commands(const char *case_name) {
    Clay_RenderCommandArray commands = Clay_EndLayout(0);
    emit_rect(case_name, "root");
    int text_index = 0;
    for (int32_t i = 0; i < commands.length; i++) {
        Clay_RenderCommand *command = Clay_RenderCommandArray_Get(&commands, i);
        if (command->commandType == CLAY_RENDER_COMMAND_TYPE_TEXT) {
            printf(
                "%s text%d %.4f %.4f %.4f %.4f\n",
                case_name,
                text_index,
                command->boundingBox.x,
                command->boundingBox.y,
                command->boundingBox.width,
                command->boundingBox.height);
            text_index++;
        }
    }
}

static void text_wraps_words(void) {
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(6, 20)}}) {
        CLAY_TEXT(CLAY_STRING("alpha beta gamma"), CLAY_TEXT_CONFIG({.fontSize = 1}));
    }
    emit_text_commands("text_wraps_words");
}

static void text_respects_newlines(void) {
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(20, 20)}}) {
        CLAY_TEXT(CLAY_STRING("alpha\nbeta"), CLAY_TEXT_CONFIG({.fontSize = 1, .wrapMode = CLAY_TEXT_WRAP_NEWLINES}));
    }
    emit_text_commands("text_respects_newlines");
}

static Clay_LayoutAlignmentX align_x_from_int(int value) {
    switch (value) {
        case 0: return CLAY_ALIGN_X_LEFT;
        case 1: return CLAY_ALIGN_X_CENTER;
        default: return CLAY_ALIGN_X_RIGHT;
    }
}

static Clay_LayoutAlignmentY align_y_from_int(int value) {
    switch (value) {
        case 0: return CLAY_ALIGN_Y_TOP;
        case 1: return CLAY_ALIGN_Y_CENTER;
        default: return CLAY_ALIGN_Y_BOTTOM;
    }
}

static void fixed_case(
    const char *case_name,
    int direction_value,
    uint16_t padding_left,
    uint16_t padding_right,
    uint16_t padding_top,
    uint16_t padding_bottom,
    uint16_t gap,
    int align_x_value,
    int align_y_value,
    float root_width,
    float root_height,
    int child_count,
    float child_widths[4],
    float child_heights[4],
    int child_width_sizing_types[4],
    int child_height_sizing_types[4],
    float child_width_sizing_values[4],
    float child_height_sizing_values[4],
    float child_width_sizing_mins[4],
    float child_height_sizing_mins[4],
    float child_width_sizing_maxes[4],
    float child_height_sizing_maxes[4],
    float child_aspect_ratios[4]) {
    const char *ids_all[] = {"root", "a", "b", "c", "d"};
    Clay_BeginLayout();
    CLAY(CLAY_ID("root"), {
        .layout = {
            .sizing = fixed_size(root_width, root_height),
            .padding = {
                .left = padding_left,
                .right = padding_right,
                .top = padding_top,
                .bottom = padding_bottom,
            },
            .childGap = gap,
            .childAlignment = {
                .x = align_x_from_int(align_x_value),
                .y = align_y_from_int(align_y_value),
            },
            .layoutDirection = direction_value == 0 ? CLAY_LEFT_TO_RIGHT : CLAY_TOP_TO_BOTTOM,
        },
    }) {
        if (child_count > 0) {
            RANDOM_CHILD(0, "a")
        }
        if (child_count > 1) {
            RANDOM_CHILD(1, "b")
        }
        if (child_count > 2) {
            RANDOM_CHILD(2, "c")
        }
        if (child_count > 3) {
            RANDOM_CHILD(3, "d")
        }
    }
    emit_case(case_name, ids_all, child_count + 1);
}

#define TREE_MAX_IDS 256
#define TREE_ID_LENGTH 32
#define TREE_MAX_TEXTS 256
#define TREE_TEXT_LENGTH 512

static char tree_emit_ids[TREE_MAX_IDS][TREE_ID_LENGTH];
static int tree_emit_id_count = 0;
static char tree_text_buffers[TREE_MAX_TEXTS][TREE_TEXT_LENGTH];
static int tree_text_count = 0;

static Clay_String clay_string_from_cstring(const char *string) {
    return (Clay_String) {
        .length = (int32_t)strlen(string),
        .chars = string,
    };
}

static void add_tree_emit_id(const char *id) {
    if (tree_emit_id_count >= TREE_MAX_IDS) {
        fprintf(stderr, "too many tree ids\n");
        exit(1);
    }
    snprintf(tree_emit_ids[tree_emit_id_count], TREE_ID_LENGTH, "%s", id);
    tree_emit_id_count++;
}

static char *allocate_tree_text_buffer(const char *case_name) {
    if (tree_text_count >= TREE_MAX_TEXTS) {
        fprintf(stderr, "too many tree text nodes in %s\n", case_name);
        exit(1);
    }
    return tree_text_buffers[tree_text_count++];
}

static Clay_TextElementConfigWrapMode text_wrap_mode_from_int(int value);
static Clay_TextAlignment text_align_from_int(int value);

static void read_text_buffer(const char *case_name, int line_count, char *text_buffer, size_t text_buffer_capacity) {
    if (line_count < 1 || line_count > 16) {
        fprintf(stderr, "invalid line count %d in %s\n", line_count, case_name);
        exit(1);
    }
    size_t offset = 0;
    for (int line_index = 0; line_index < line_count; line_index++) {
        int word_count;
        if (scanf("%d", &word_count) != 1) {
            fprintf(stderr, "missing word count in %s\n", case_name);
            exit(1);
        }
        if (word_count < 1 || word_count > 16) {
            fprintf(stderr, "invalid word count %d in %s\n", word_count, case_name);
            exit(1);
        }
        if (line_index > 0 && offset + 1 < text_buffer_capacity) {
            text_buffer[offset++] = '\n';
        }
        for (int word_index = 0; word_index < word_count; word_index++) {
            int word_length;
            if (scanf("%d", &word_length) != 1) {
                fprintf(stderr, "missing word length in %s\n", case_name);
                exit(1);
            }
            if (word_index > 0 && offset + 1 < text_buffer_capacity) {
                text_buffer[offset++] = ' ';
            }
            for (int char_index = 0; char_index < word_length && offset + 1 < text_buffer_capacity; char_index++) {
                text_buffer[offset++] = 'x';
            }
        }
    }
    text_buffer[offset] = '\0';
}

static Clay_ElementDeclaration tree_declaration(
    int direction_value,
    unsigned int padding_left,
    unsigned int padding_right,
    unsigned int padding_top,
    unsigned int padding_bottom,
    unsigned int gap,
    int align_x_value,
    int align_y_value,
    int width_sizing_type,
    int height_sizing_type,
    float width_sizing_value,
    float height_sizing_value,
    float width_sizing_min,
    float height_sizing_min,
    float width_sizing_max,
    float height_sizing_max,
    float aspect_ratio,
    int clip_horizontal,
    int clip_vertical,
    float child_offset_x,
    float child_offset_y) {
    return CLAY__INIT(Clay_ElementDeclaration) {
        .layout = {
            .sizing = {
                .width = sizing_axis_from_int(width_sizing_type, width_sizing_value, width_sizing_min, width_sizing_max),
                .height = sizing_axis_from_int(height_sizing_type, height_sizing_value, height_sizing_min, height_sizing_max),
            },
            .padding = {
                .left = (uint16_t)padding_left,
                .right = (uint16_t)padding_right,
                .top = (uint16_t)padding_top,
                .bottom = (uint16_t)padding_bottom,
            },
            .childGap = (uint16_t)gap,
            .childAlignment = {
                .x = align_x_from_int(align_x_value),
                .y = align_y_from_int(align_y_value),
            },
            .layoutDirection = direction_value == 0 ? CLAY_LEFT_TO_RIGHT : CLAY_TOP_TO_BOTTOM,
        },
        .aspectRatio = {.aspectRatio = aspect_ratio},
        .clip = {
            .horizontal = clip_horizontal != 0,
            .vertical = clip_vertical != 0,
            .childOffset = {child_offset_x, child_offset_y},
        },
    };
}

static void add_intrinsic_leaf(float width, float height) {
    Clay__OpenElement();
    Clay__ConfigureOpenElement(CLAY__INIT(Clay_ElementDeclaration) {
        .layout = {.sizing = fixed_size(width, height)},
    });
    Clay__CloseElement();
}

static void add_text_leaf(const char *case_name) {
    int wrap_mode_value;
    int text_align_value;
    int line_count;
    if (scanf("%d %d %d", &wrap_mode_value, &text_align_value, &line_count) != 3) {
        fprintf(stderr, "missing tree text payload in %s\n", case_name);
        exit(1);
    }
    char *text_buffer = allocate_tree_text_buffer(case_name);
    read_text_buffer(case_name, line_count, text_buffer, TREE_TEXT_LENGTH);
    CLAY_TEXT(
        clay_string_from_cstring(text_buffer),
        CLAY_TEXT_CONFIG({
            .fontSize = 1,
            .wrapMode = text_wrap_mode_from_int(wrap_mode_value),
            .textAlignment = text_align_from_int(text_align_value),
        }));
}

static void read_tree_node(const char *case_name) {
    char id[TREE_ID_LENGTH];
    int child_count;
    int node_kind;
    float intrinsic_width;
    float intrinsic_height;
    int direction_value;
    unsigned int padding_left;
    unsigned int padding_right;
    unsigned int padding_top;
    unsigned int padding_bottom;
    unsigned int gap;
    int align_x_value;
    int align_y_value;
    int width_sizing_type;
    int height_sizing_type;
    float width_sizing_value;
    float height_sizing_value;
    float width_sizing_min;
    float height_sizing_min;
    float width_sizing_max;
    float height_sizing_max;
    float aspect_ratio;
    int clip_horizontal;
    int clip_vertical;
    float child_offset_x;
    float child_offset_y;

    if (scanf(
            "%31s %d %f %f %d %u %u %u %u %u %d %d %d %d %f %f %f %f %f %f %f %d %d %d %f %f",
            id,
            &child_count,
            &intrinsic_width,
            &intrinsic_height,
            &direction_value,
            &padding_left,
            &padding_right,
            &padding_top,
            &padding_bottom,
            &gap,
            &align_x_value,
            &align_y_value,
            &width_sizing_type,
            &height_sizing_type,
            &width_sizing_value,
            &height_sizing_value,
            &width_sizing_min,
            &height_sizing_min,
            &width_sizing_max,
            &height_sizing_max,
            &aspect_ratio,
            &node_kind,
            &clip_horizontal,
            &clip_vertical,
            &child_offset_x,
            &child_offset_y)
        != 26) {
        fprintf(stderr, "missing tree node in %s\n", case_name);
        exit(1);
    }
    if (child_count < 0 || child_count > 4) {
        fprintf(stderr, "invalid tree child count %d in %s\n", child_count, case_name);
        exit(1);
    }
    if (node_kind < 0 || node_kind > 2) {
        fprintf(stderr, "invalid tree node kind %d in %s\n", node_kind, case_name);
        exit(1);
    }
    if (node_kind != 2 && child_count != 0) {
        fprintf(stderr, "tree leaf node kind %d has child count %d in %s\n", node_kind, child_count, case_name);
        exit(1);
    }
    if (node_kind == 2 && child_count == 0) {
        fprintf(stderr, "tree container has no children in %s\n", case_name);
        exit(1);
    }

    add_tree_emit_id(id);
    Clay__OpenElementWithId(Clay_GetElementId(clay_string_from_cstring(id)));
    Clay__ConfigureOpenElement(
        tree_declaration(
            direction_value,
            padding_left,
            padding_right,
            padding_top,
            padding_bottom,
            gap,
            align_x_value,
            align_y_value,
            width_sizing_type,
            height_sizing_type,
            width_sizing_value,
            height_sizing_value,
            width_sizing_min,
            height_sizing_min,
            width_sizing_max,
            height_sizing_max,
            aspect_ratio,
            clip_horizontal,
            clip_vertical,
            child_offset_x,
            child_offset_y));
    if (node_kind == 1) {
        add_text_leaf(case_name);
    } else if (child_count == 0) {
        add_intrinsic_leaf(intrinsic_width, intrinsic_height);
    } else {
        for (int i = 0; i < child_count; i++) {
            read_tree_node(case_name);
        }
    }
    Clay__CloseElement();
}

static void run_tree_stdin_cases(void) {
    char case_name[64];
    while (scanf("%63s", case_name) == 1) {
        tree_emit_id_count = 0;
        tree_text_count = 0;
        Clay_BeginLayout();
        read_tree_node(case_name);
        Clay_EndLayout(0);
        for (int i = 0; i < tree_emit_id_count; i++) {
            emit_rect(case_name, tree_emit_ids[i]);
        }
    }
}

static void emit_tree_debug_row(const char *case_name, const char *phase, const char *id) {
    Clay_LayoutElementHashMapItem *item = Clay__GetHashMapItem(Clay_GetElementId(clay_string_from_cstring(id)).id);
    if (!item || item == &Clay_LayoutElementHashMapItem_DEFAULT) {
        fprintf(stderr, "missing debug element %s in case %s\n", id, case_name);
        exit(1);
    }
    Clay_LayoutElement *element = item->layoutElement;
    printf(
        "%s %s %s %.4f %.4f %.4f %.4f\n",
        case_name,
        phase,
        id,
        element->dimensions.width,
        element->dimensions.height,
        element->minDimensions.width,
        element->minDimensions.height);
}

static void run_tree_debug_stdin_cases(void) {
    char case_name[64];
    while (scanf("%63s", case_name) == 1) {
        tree_emit_id_count = 0;
        tree_text_count = 0;
        Clay_BeginLayout();
        read_tree_node(case_name);
        for (int i = 0; i < tree_emit_id_count; i++) {
            emit_tree_debug_row(case_name, "closed", tree_emit_ids[i]);
        }
        Clay_EndLayout(0);
        for (int i = 0; i < tree_emit_id_count; i++) {
            emit_tree_debug_row(case_name, "final", tree_emit_ids[i]);
        }
    }
}

static Clay_TextElementConfigWrapMode text_wrap_mode_from_int(int value) {
    switch (value) {
        case 1: return CLAY_TEXT_WRAP_NEWLINES;
        case 2: return CLAY_TEXT_WRAP_NONE;
        default: return CLAY_TEXT_WRAP_WORDS;
    }
}

static Clay_TextAlignment text_align_from_int(int value) {
    switch (value) {
        case 1: return CLAY_TEXT_ALIGN_CENTER;
        case 2: return CLAY_TEXT_ALIGN_RIGHT;
        default: return CLAY_TEXT_ALIGN_LEFT;
    }
}

static void run_text_stdin_cases(void) {
    char case_name[64];
    float root_width;
    float root_height;
    int wrap_mode_value;
    int text_align_value;
    int line_count;
    while (scanf("%63s %f %f %d %d %d", case_name, &root_width, &root_height, &wrap_mode_value, &text_align_value, &line_count) == 6) {
        char text_buffer[512];
        read_text_buffer(case_name, line_count, text_buffer, sizeof(text_buffer));
        Clay_BeginLayout();
        CLAY(CLAY_ID("root"), {.layout = {.sizing = fixed_size(root_width, root_height)}}) {
            CLAY_TEXT(
                clay_string_from_cstring(text_buffer),
                CLAY_TEXT_CONFIG({
                    .fontSize = 1,
                    .wrapMode = text_wrap_mode_from_int(wrap_mode_value),
                    .textAlignment = text_align_from_int(text_align_value),
                }));
        }
        emit_text_commands(case_name);
    }
}

static void run_stdin_cases(void) {
    char case_name[64];
    int direction_value;
    unsigned int padding_left;
    unsigned int padding_right;
    unsigned int padding_top;
    unsigned int padding_bottom;
    unsigned int gap;
    int align_x_value;
    int align_y_value;
    float root_width;
    float root_height;
    int child_count;
    while (scanf(
               "%63s %d %u %u %u %u %u %d %d %f %f %d",
               case_name,
               &direction_value,
               &padding_left,
               &padding_right,
               &padding_top,
               &padding_bottom,
               &gap,
               &align_x_value,
               &align_y_value,
               &root_width,
               &root_height,
               &child_count)
           == 12) {
        if (child_count < 0 || child_count > 4) {
            fprintf(stderr, "invalid child count %d in %s\n", child_count, case_name);
            exit(1);
        }
        float child_widths[4] = {0};
        float child_heights[4] = {0};
        int child_width_sizing_types[4] = {1, 1, 1, 1};
        int child_height_sizing_types[4] = {1, 1, 1, 1};
        float child_width_sizing_values[4] = {0};
        float child_height_sizing_values[4] = {0};
        float child_width_sizing_mins[4] = {0};
        float child_height_sizing_mins[4] = {0};
        float child_width_sizing_maxes[4] = {0};
        float child_height_sizing_maxes[4] = {0};
        float child_aspect_ratios[4] = {0};
        for (int i = 0; i < child_count; i++) {
            if (scanf(
                    "%f %f %d %d %f %f %f %f %f %f %f",
                    &child_widths[i],
                    &child_heights[i],
                    &child_width_sizing_types[i],
                    &child_height_sizing_types[i],
                    &child_width_sizing_values[i],
                    &child_height_sizing_values[i],
                    &child_width_sizing_mins[i],
                    &child_height_sizing_mins[i],
                    &child_width_sizing_maxes[i],
                    &child_height_sizing_maxes[i],
                    &child_aspect_ratios[i]) != 11) {
                fprintf(stderr, "missing child size in %s\n", case_name);
                exit(1);
            }
        }
        fixed_case(
            case_name,
            direction_value,
            (uint16_t)padding_left,
            (uint16_t)padding_right,
            (uint16_t)padding_top,
            (uint16_t)padding_bottom,
            (uint16_t)gap,
            align_x_value,
            align_y_value,
            root_width,
            root_height,
            child_count,
            child_widths,
            child_heights,
            child_width_sizing_types,
            child_height_sizing_types,
            child_width_sizing_values,
            child_height_sizing_values,
            child_width_sizing_mins,
            child_height_sizing_mins,
            child_width_sizing_maxes,
            child_height_sizing_maxes,
            child_aspect_ratios);
    }
}

int main(int argc, char **argv) {
    init_clay();
    if (argc > 1 && strcmp(argv[1], "--tree-debug-stdin") == 0) {
        run_tree_debug_stdin_cases();
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--tree-stdin") == 0) {
        run_tree_stdin_cases();
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--text-stdin") == 0) {
        run_text_stdin_cases();
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--stdin") == 0) {
        run_stdin_cases();
        return 0;
    }
    row_gap_and_padding();
    column_gap_and_padding();
    fixed_box_centers_child();
    percent_child();
    grow_main_axis();
    grow_cross_axis();
    clamp_grow();
    aspect_ratio_width_drives_height();
    aspect_ratio_height_drives_width();
    unequal_grow_main_axis();
    nested_box_positions_children();
    overflow_cross_center();
    clip_main_axis_does_not_compress();
    clip_cross_axis_grows_to_content();
    clip_cross_axis_uses_pre_percent_inner_size();
    clip_child_offset_places_children();
    text_wraps_words();
    text_respects_newlines();
    return 0;
}
