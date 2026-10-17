cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.49.0"
  sha256 arm:   "fb308feda917ffe98d0204a48ee15372689fae30c68e7f435cffaa6f31b398e4",
         intel: "0770bf7481d957d764b8b186a025caf2f0e475851f26dda794ef82def76d8108"

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
