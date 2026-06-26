cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.34.0"
  sha256 arm:   "6ca001de39eee8c071781a3a47beaef732bb4e8b86d3d19224d25d79cb1d519f",
         intel: "b9be18dbbd0c8c3fe6b9792633c5a085f1e271af267fadc7dc81b3d1eae8b9c7"

  url "https://github.com/paulrobello/par-term/releases/download/v#{version}/par-term-macos-#{arch}.zip"
  name "par-term"
  desc "Cross-platform GPU-accelerated terminal emulator with inline graphics support"
  homepage "https://github.com/paulrobello/par-term"

  depends_on macos: ">= :catalina"

  livecheck do
    url :homepage
    strategy :github_latest
  end

  app "par-term.app"

  zap trash: [
    "~/Library/Application Support/par-term",
    "~/Library/Preferences/com.paulrobello.par-term.plist",
    "~/Library/Saved Application State/com.paulrobello.par-term.savedState",
    "~/.config/par-term",
  ]
end
