/*
 * NuttX modlib 构造/析构胶水。
 *
 * 模块被拷入可写 RAM 后，ctor 先解码 verifier 要求的 opaque 常量字，
 * 再调用 prepare 并向 /dev/canopus 注册模块描述符。
 *
 * 三个 rodata 锚点供 encode-opaque-words.py 做节内相对偏移；必须：
 * 1) 位于各输出节偏移 0；2) 内容不得形似固件地址；3) 用 asm 钉住防 gc。
 * `.rodata.cst8` 是较新 LLVM 字面量池节，Windows nightly 工具链会生成。
 */
#include <stdint.h>

__attribute__((section(".rodata"), used, aligned(4)))
const uint8_t canopus_rodata_anchor[4] = {0};

/* 非空、唯一的 4 字节锚点，避免 SHF_MERGE 折叠或重排。 */
__attribute__((section(".rodata.str1.1"), used, aligned(4)))
const uint8_t canopus_rodata_str1_1_anchor[4] = {0xA7, 0x5C, 0xD3, 0x00};

__attribute__((section(".rodata.cst8"), used, aligned(8)))
const uint8_t canopus_rodata_cst8_anchor[8] = {0x5C, 0xA7, 0xD3, 0x00, 0x37, 0x1A, 0x9C, 0x00};

extern void canopus_decode_opaque_words(void) __attribute__((weak));

__attribute__((constructor)) static void canopus_mod_ctor(void)
{
    extern int canopus_mod_prepare(const void *ctx);
    extern int canopus_register_module_descriptor(void);

    __asm__ volatile("" : : "r"(canopus_rodata_anchor),
                     "r"(canopus_rodata_str1_1_anchor),
                     "r"(canopus_rodata_cst8_anchor) : "memory");

    if (canopus_decode_opaque_words != 0) {
        canopus_decode_opaque_words();
    }
    (void)canopus_mod_prepare(0);
    (void)canopus_register_module_descriptor();
}

__attribute__((destructor)) static void canopus_mod_dtor(void)
{
    extern int canopus_mod_stop(const void *ctx);
    (void)canopus_mod_stop(0);
}
