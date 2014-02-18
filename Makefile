include rust-lua/common.mk
RUST_LUA := rust-lua/$(LIBNAME)

RUST_IRC := rust-irclib/$(shell rustc --crate-file-name rust-irclib/lib.rs)

RUST_TOML := rust-toml/lib/$(shell rustc --crate-file-name rust-toml/src/toml/lib.rs)

PKGNAME := $(shell rustc --crate-file-name pkg.rs)

RUSTC_FLAGS := $(if $(DEBUG),-g)

.PHONY: all clean test

all: $(PKGNAME)

$(PKGNAME): $(RUST_LUA) $(RUST_IRC) $(RUST_TOML)
	rustc $(RUSTC_FLAGS) --dep-info pkg.d -L rust-lua -L rust-irclib -L rust-toml/lib pkg.rs

include pkg.d

define REBUILD_DIR
.PHONY: $(1)
$(1):
	$(MAKE) -C $(2) lib
endef

$(foreach lib,$(RUST_LUA) $(RUST_IRC) $(RUST_TOML),\
  $(if $(shell $(MAKE) -C $(firstword $(subst /, ,$(lib))) -q lib || echo no),\
       $(eval $(call REBUILD_DIR,$(lib),$(firstword $(subst /, ,$(lib)))))))

clean:
	-rm -f $(PKGNAME)
	-$(MAKE) -C $(dir $(RUST_LUA)) clean
	-$(MAKE) -C $(dir $(RUST_IRC)) clean
	-$(MAKE) -C $(firstword $(subst /, ,$(RUST_TOML))) clean
