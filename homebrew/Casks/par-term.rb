cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.43.0"
  sha256 arm:   "058614c59356e03894e1a5f6d43ba3f3de55bade3c0c3bd2f90a815a9dd9e22f",
         intel: "ff86702c570252b46d91ae8a9108c5c73f68b02745df057b14825557519e4217"

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
