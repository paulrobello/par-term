cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.11.0"
  sha256 arm:   "e2d5f145e2640188ffd3a161da5a5919cf19e8c27277ef5d1a08f412baaae5d0",
         intel: "040a9ba488f2fc4095089c8a7b03fc0f3ea2a3f3b37ee1dfac0e150e30c24851"

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
