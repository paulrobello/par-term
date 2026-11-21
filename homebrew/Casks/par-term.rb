cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.55.0"
  sha256 arm:   "2854a45613d50dafbef14b93d58f301e340d4f1e73fcd599fdcdf36955621858",
         intel: "c9e8e275eee50157dbb8229596e6aab1fc2f69f23d7981a8f44f828aabdeaff0"

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
