include rust-lua/common.mk
RUST_LUA := rust-lua/$(LIBNAME)

RUST_IRC := rust-irclib/$(shell rustc --crate-file-name rust-irclib/lib.rs)

RUST_TOML_DIR := rust-toml/src/toml/
RUST_TOML := .build/$(shell rustc --crate-file-name $(RUST_TOML_DIR)/lib.rs)
RUST_TOML_FILES := $(wildcard $(RUST_TOML_DIR)/*.rs)

PKGNAME := $(shell rustc --crate-file-name pkg.rs)

.PHONY: all clean test

all: $(PKGNAME)

$(PKGNAME): $(RUST_LUA) $(RUST_IRC) $(RUST_TOML)
	rustc --dep-info pkg.d -L rust-lua -L rust-irclib -L .build pkg.rs

include pkg.d

define REBUILD_DIR
.PHONY: $(1)
$(1):
	$(MAKE) -C $(dir $(1))
endef

$(foreach lib,$(RUST_LUA) $(RUST_IRC),\
  $(if $(shell $(MAKE) -C $(dir $(lib)) -q || echo no),\
       $(eval $(call REBUILD_DIR,$(lib)))))

$(RUST_TOML): $(RUST_TOML_FILES)
	mkdir -p .build
	rustc -O --out-dir .build --rlib $(RUST_TOML_DIR)/lib.rs

clean:
	-rm -f $(PKGNAME)
	-rm -rf .build
	-$(MAKE) -C $(dir $(RUST_LUA)) clean
	-$(MAKE) -C $(dir $(RUST_IRC)) clean
