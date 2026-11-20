cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.54.0"
  sha256 arm:   "d5a7db1c59e26074bd73e19481d38632934dc7863a0fc3e56abd1f1ac09ea3b0",
         intel: "defd78295490dde135dce329e4e98267c8d4b97fc28581de3b551793c7d9a30a"

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
