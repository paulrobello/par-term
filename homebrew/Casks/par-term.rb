cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.28.0"
  sha256 arm:   "4a6665247ca40ad06830fc0cc7b1c00c91a39b18327680621df59ce47f6deca9",
         intel: "c1687224d5ad94fd57b776823532c04766499f92437e3de606dfae19ad8870fc"

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
