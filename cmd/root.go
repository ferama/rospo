package cmd

import "github.com/spf13/cobra"

var (
	rootCmd = &cobra.Command{
		Use:  "rospo",
		Long: "Tool to create relieable ssh tunnels.",
	}
)

// Execute executes the root command
func Execute() error {

	return rootCmd.Execute()
}
