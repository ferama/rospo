package cmd

import (
	"fmt"
	"os"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/spf13/cobra"
)

// Version is the actual rospo version. This value
// is set during the build process using -ldflags="-X 'github.com/ferama/rospo/cmd.Version=
var Version = "development"

func init() {
	rootCmd.PersistentFlags().BoolP("quiet", "q", false, "if set disable all logs")
}

var rootCmd = &cobra.Command{
	Use:     "rospo",
	Long:    "The tool to create relieable ssh tunnels.",
	Version: Version,
	Args:    cobra.MinimumNArgs(1),
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		if quiet, _ := cmd.Flags().GetBool("quiet"); quiet {
			logger.DisableLoggers()
		}
	},
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("invalid subcommand")
		os.Exit(1)
	},
}

// Execute executes the root command
func Execute() error {
	return rootCmd.Execute()
}
