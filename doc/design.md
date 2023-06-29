# Design Decisions

- Extract database interactions into dedicated struct/file ✅
- "Interaction" flow will be done by an inquire module
	- module will not use diesel, but instead the database module
- Output trait for printing messages/results
	- inquire module will use high level datatypes to interact with output module
	  (e.g. TttError::NoProjectDefined if user wants to start a frame without available projects)
