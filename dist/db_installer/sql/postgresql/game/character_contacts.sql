DROP TABLE IF EXISTS `character_contacts`;
CREATE TABLE IF NOT EXISTS `character_contacts` (
  charId INT NOT NULL DEFAULT 0,
  contactId INT NOT NULL DEFAULT 0,
  PRIMARY KEY (`charId`,`contactId`)
) ;