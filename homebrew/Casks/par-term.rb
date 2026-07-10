cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.36.0"
  sha256 arm:   "b49fe62e648b11a53c6efe23a46d50320e484fa6fdc27d12505057d474bfd24d",
         intel: "e1b94d3e9e68dc16edb4a81a9d925da042d797c330b5866fce5088f9d521127d"

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
