{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "zfile";
  version = "unstable";
  src = ./..;

  cargoHash = "sha256-1v7Ep9yWRrgZ9htSGVkEL7Ij/DTpa1+tkWQd7CpJiVQ=";

  meta = {
    description = "blazingly fast fm";
    homepage = "https://github.com/JuleeC/zfile";
    license = lib.licenses.mit;
    maintainers = [ ];
  };
}
