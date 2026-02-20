cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.20.0"
  sha256 arm:   "5537f73dde09169208adc7d2f1070fb190c66ee8e8fa08a04c904e2f3600f265",
         intel: "9aff0e6e9b83cc10c0a3aaee3d787daf9431e58bdfc95f60d71414989111915c"

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
