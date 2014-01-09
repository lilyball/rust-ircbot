include rust-lua/common.mk
RUST_LUA := rust-lua/$(LIBNAME)

RUST_IRC := rust-irclib/$(shell rustc --crate-file-name rust-irclib/lib.rs)

PKGNAME := $(shell rustc --crate-file-name pkg.rs)

.PHONY: all clean test
.DEFAULT: all

all: $(PKGNAME)

$(PKGNAME): $(RUST_LUA) $(RUST_IRC)
	rustc --dep-info pkg.d -L rust-lua -L rust-irclib pkg.rs

include pkg.d

define REBUILD_DIR
.PHONY: $(1)
$(1):
	$(MAKE) -C $(dir $(1))
endef

$(foreach lib,$(RUST_LUA) $(RUST_IRC),\
  $(if $(shell $(MAKE) -C $(dir $(lib)) -q || echo no),\
       $(eval $(call REBUILD_DIR,$(lib)))))

clean:
	-rm -f $(PKGNAME)
	-$(MAKE) -C $(dir $(RUST_LUA)) clean
	-$(MAKE) -C $(dir $(RUST_IRC)) clean
