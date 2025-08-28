{ pkgs }:
{
  default = pkgs.mkShell {
    packages = builtins.attrValues {
      inherit (pkgs)
        git
        ;
    };

    nativeBuildInputs = builtins.attrValues {
      inherit (pkgs)
        cargo
        rustc
        ;
    };
  };
}
