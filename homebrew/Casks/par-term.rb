cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.40.0"
  sha256 arm:   "7d63e4ce091007491c730e11f01b1ed2e143ee84fda9eaaf583d2994cc280fde",
         intel: "b177b2367d90bc946f16d20c1a80a46055c00e678de486c4c9e76f0242304c7e"

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
