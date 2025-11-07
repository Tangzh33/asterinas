{ pkgs }:

let
  xorg = pkgs.callPackage ./xorg.nix {
    inherit pkgs;
  };

in rec {
  xfwm4 = pkgs.xfce.xfwm4.overrideAttrs (oldAttrs: {
    version = "4.16.1";
  });

  xfdesktop = pkgs.xfce.xfdesktop.overrideAttrs (oldAttrs: {
    version = "4.16.0";
    patches = (oldAttrs.patches or []) ++ [
      ./patches/xfdesktop4/0001-Fix-not-using-consistent-monitor-identifiers.patch
      ./patches/xfdesktop4/0001-Hardcode-thuna-as-the-filesystem-manager.patch
    ];
  });

  vte_debug = pkgs.vte.overrideAttrs (oldAttrs: {
    patches = (oldAttrs.patches or []) ++ [
      ./patches/vte-debug/0001-Debug.patch
    ];
    mesonFlags = (oldAttrs.mesonFlags or []) ++ [
      "-Ddbg=true"
      "-Dc_args=-UTIOCPKT"
    ];
  });

  xfterminal = (pkgs.xfce.xfce4-terminal.override { vte = vte_debug; })
    .overrideAttrs (oldAttrs: {
      patches = (oldAttrs.patches or []) ++ [
        ./patches/xfterminal/0001-Debug.patch
      ];
  });
}