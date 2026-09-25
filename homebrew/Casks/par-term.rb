cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.46.0"
  sha256 arm:   "10a4b0911a184bbebb2653188227ed17047b07938c874f149c12be730d58848c",
         intel: "b5be9a3edf036444fbb5c972eceac2ef1c8e366424277c1f9599cf9d5642be89"

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
