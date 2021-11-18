{
  description = "nyat64";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";

  outputs = { self, nixpkgs }: let
    overlay = final: prev: {
      nyat64 = final.callPackage (
        { rustPlatform }:

        rustPlatform.buildRustPackage {
          pname = "nyat64";
          version = self.shortRev or "dirty-${toString self.lastModifiedDate}";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
        }
      ) {};
    };
  in {
    inherit overlay;
    packages.x86_64-linux = import nixpkgs {
      system = "x86_64-linux";
      overlays = [ overlay ];
    };
    defaultPackage.x86_64-linux = self.packages.x86_64-linux.nyat64;
  };
}
