cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.27.0"
  sha256 arm:   "48ee59159fb56ba616c3d6509bd4152da5c44125ee97a3485b9cb9c4cfa44966",
         intel: "25f1c702e4bd09cc67efaefdead9d5ff99fd3d44a6a1e0d8a2316e8762f78204"

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
