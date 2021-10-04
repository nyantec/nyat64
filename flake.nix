{
  description = "nyat64";

  outputs = { self, nixpkgs }: let
    version = self.shortRev or (toString self.lastModifiedDate);
    overlay = final: prev: {
      nyat64 = final.callPackage (
        { rustPlatform }: rustPlatform.buildRustPackage {
          pname = "nyat64";
          inherit version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
        }
      ) {};

      nyat64-pkg = final.callPackage (
        { nyat64, zstd }: pkgs.runCommand "nyat64-pkg" {
          nativeBuildInputs = [ zstd ];
        } ''
          mkdir -p usr/bin $out
          cp ${nyat64}/bin/nyat64 usr/bin
          tar --zstd -cf $out/nyat64-x86_64-${nyat64.version}.pkg usr
        ''
      ) {};
    };
    pkgs = import nixpkgs {
      system = "x86_64-linux";
      crossSystem = {
        isStatic = true;
        config = "x86_64-unknown-linux-musl";
      };
      overlays = [ overlay ];
    };
  in {
    inherit overlay;
    packages.x86_64-linux = {
      inherit (pkgs) nyat64 nyat64-pkg;
    };
    defaultPackage.x86_64-linux = self.packages.x86_64-linux.nyat64-pkg;
  };
}
