cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.47.0"
  sha256 arm:   "c47ba4f80ba85557d8b5312572f4603d757217c15cc1a80b5b8087bc98504259",
         intel: "31c70210addf64a8a8b993099f7d31070cac8f32ecbead3864d3b90961445261"

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
