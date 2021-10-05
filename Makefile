NET_ROUTER ?= router
#NET_SRC ?= src
NET_DST ?= dst
RUST_LOG ?= "nyat64=info,nyat64::*=info,tun=info,tun::*=info"
CARGO_FLAGS ?=
CARGO ?= cargo

srcdir = $(realpath .)

debug-bin = "$(srcdir)/target/debug/nyat64"
release-bin = "$(srcdir)/target/release/nyat64"

ifdef FEATURES
  CARGO_FLAGS += --no-default-features --features $(FEATURES)
  .PHONY += $(debug-bin)
  .PHONY += $(release-bin)
endif

include netns.makefile
-include $(debug-bin).d
-include $(release-bin).d

clean:
	rm -r= target/
.PHONY += clean

$(release-bin):
	$(CARGO) build --release $(CARGO_FLAGS)

$(debug-bin):
	$(CARGO) build $(CARGO_FLAGS)

build: $(release-bin)
debug: $(debug-bin)

all: $(debug-bin) $(release-bin)