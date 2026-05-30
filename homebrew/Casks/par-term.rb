cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.32.1"
  sha256 arm:   "062b18376da3f35f799ed81c50725e07ba0061423cfed989eb9bc1f8a2b09515",
         intel: "4fcea2245ff8464a42ee1da3484094caf90740598569852ca0ce8de57f5818a4"

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
