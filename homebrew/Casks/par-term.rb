cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.57.0"
  sha256 arm:   "8349a77da1f44df4b5919f18ce5bbc2daa85dea48ccbdad25b33dc151ec679cb",
         intel: "f7b68347cc4d0674097be9d6af3e1075b96f4fdc160934e735607bec22018e9b"

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
