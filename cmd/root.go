package cmd

import "github.com/spf13/cobra"

var (
	rootCmd = &cobra.Command{
		Use:  "rospo",
		Long: "The tool to create relieable ssh tunnels.",
	}
)

// Execute executes the root command
func Execute() error {

	return rootCmd.Execute()
}
