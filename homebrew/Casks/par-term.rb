cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.45.0"
  sha256 arm:   "8411ac65d6e630f942a7684d6a4483985be0d5c27b119644a9530bad8d65ef7c",
         intel: "7bdc6cb9feb5f31b0641ffd9265ed618d165aa74a0457a842a4b8ec5929d0648"

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
